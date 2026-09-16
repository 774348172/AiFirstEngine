use crate::architecture_debt::{
    reconcile_debt, ArchitectureDebtLedger, DebtReconciliation, ObservedDebt,
};
use crate::architecture_policy::ArchitectureDiagnostic;
use crate::cargo_json::sha256_hex;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CHANGE_SCOPE_SCHEMA_VERSION: &str = "architecture-change-scope.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeScopeRequest {
    pub schema_version: String,
    pub base_commit: String,
    pub head_commit: String,
    pub dirty_patch_digest: Option<String>,
    pub merge_base_policy: MergeBasePolicy,
    pub accepted_merge_base: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeBasePolicy {
    ExactBase,
    AcceptedMergeBase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalSubject {
    pub path: String,
    pub content_digest: String,
    pub ast_digest: String,
    pub dependency_digest: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClassification {
    Unchanged,
    NonSemantic,
    Semantic,
    RenameMove,
    Added,
    Removed,
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSubject {
    pub before_path: Option<String>,
    pub after_path: Option<String>,
    pub identity_digest: String,
    pub classification: ChangeClassification,
    pub changed_symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeScope {
    pub schema_version: String,
    pub base_commit: String,
    pub head_commit: String,
    pub dirty_patch_digest: Option<String>,
    pub subjects: Vec<ChangedSubject>,
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

pub fn validate_change_request(
    request: &ChangeScopeRequest,
) -> Result<(), Box<ArchitectureDiagnostic>> {
    if request.schema_version != CHANGE_SCOPE_SCHEMA_VERSION {
        return Err(Box::new(change_diagnostic(
            "change_scope.unknown_schema",
            "unsupported change scope schema",
            "Use architecture-change-scope.v1.",
        )));
    }
    if !valid_commit(&request.base_commit) || !valid_commit(&request.head_commit) {
        return Err(Box::new(change_diagnostic(
            "change_scope.unknown_base",
            "base_commit and head_commit must be exact 40-character Git object IDs",
            "Resolve and record exact local Git commits before review.",
        )));
    }
    if request.base_commit == request.head_commit && request.dirty_patch_digest.is_none() {
        return Err(Box::new(change_diagnostic(
            "change_scope.empty",
            "base and head are identical and no dirty patch digest was supplied",
            "Supply a real committed range or a canonical dirty patch digest.",
        )));
    }
    if request
        .dirty_patch_digest
        .as_deref()
        .is_some_and(|digest| !valid_digest(digest))
    {
        return Err(Box::new(change_diagnostic(
            "change_scope.patch_digest_invalid",
            "dirty_patch_digest is not a SHA-256 digest",
            "Hash the canonical patch bytes with SHA-256.",
        )));
    }
    match request.merge_base_policy {
        MergeBasePolicy::ExactBase if request.accepted_merge_base.is_some() => {
            Err(Box::new(change_diagnostic(
                "change_scope.merge_base_ambiguous",
                "ExactBase cannot include accepted_merge_base",
                "Remove accepted_merge_base or select AcceptedMergeBase.",
            )))
        }
        MergeBasePolicy::AcceptedMergeBase
            if !request
                .accepted_merge_base
                .as_deref()
                .is_some_and(valid_commit) =>
        {
            Err(Box::new(change_diagnostic(
                "change_scope.merge_base_ambiguous",
                "AcceptedMergeBase requires an exact accepted_merge_base commit",
                "Record the reviewed merge base commit.",
            )))
        }
        _ => Ok(()),
    }
}

pub fn canonical_rust_subject(
    path: impl Into<String>,
    source: &str,
    dependencies: &[String],
) -> Result<CanonicalSubject, Box<ArchitectureDiagnostic>> {
    let path = normalize_path(&path.into());
    let syntax = syn::parse_file(source).map_err(|error| {
        let mut diagnostic = change_diagnostic(
            "change_scope.rust_parse_failed",
            error.to_string(),
            "Repair Rust syntax before computing canonical change scope.",
        );
        diagnostic.source_path = Some(path.clone());
        Box::new(diagnostic)
    })?;
    let canonical_ast = syntax.to_token_stream().to_string();
    let mut symbols = syntax
        .items
        .iter()
        .filter_map(item_symbol)
        .collect::<Vec<_>>();
    symbols.sort();
    let mut dependencies = dependencies.to_vec();
    dependencies.sort();
    dependencies.dedup();
    Ok(CanonicalSubject {
        path,
        content_digest: digest(source.as_bytes()),
        ast_digest: digest(canonical_ast.as_bytes()),
        dependency_digest: digest(dependencies.join("\n").as_bytes()),
        symbols,
    })
}

pub fn compute_change_scope(
    request: &ChangeScopeRequest,
    before: &[CanonicalSubject],
    after: &[CanonicalSubject],
) -> ChangeScope {
    let mut diagnostics = Vec::new();
    if let Err(diagnostic) = validate_change_request(request) {
        diagnostics.push(*diagnostic);
        return ChangeScope {
            schema_version: CHANGE_SCOPE_SCHEMA_VERSION.to_string(),
            base_commit: request.base_commit.clone(),
            head_commit: request.head_commit.clone(),
            dirty_patch_digest: request.dirty_patch_digest.clone(),
            subjects: Vec::new(),
            diagnostics,
        };
    }

    let before_by_path = before
        .iter()
        .map(|subject| (subject.path.as_str(), subject))
        .collect::<BTreeMap<_, _>>();
    let after_by_path = after
        .iter()
        .map(|subject| (subject.path.as_str(), subject))
        .collect::<BTreeMap<_, _>>();
    let mut matched_after = BTreeSet::new();
    let mut subjects = Vec::new();

    for before_subject in before {
        if let Some(after_subject) = after_by_path.get(before_subject.path.as_str()).copied() {
            matched_after.insert(after_subject.path.as_str());
            subjects.push(compare_subjects(before_subject, after_subject, false));
            continue;
        }
        let rename_candidates = after
            .iter()
            .filter(|candidate| {
                !matched_after.contains(candidate.path.as_str())
                    && candidate.content_digest == before_subject.content_digest
            })
            .collect::<Vec<_>>();
        match rename_candidates.as_slice() {
            [after_subject] => {
                matched_after.insert(after_subject.path.as_str());
                subjects.push(compare_subjects(before_subject, after_subject, true));
            }
            [] => subjects.push(ChangedSubject {
                before_path: Some(before_subject.path.clone()),
                after_path: None,
                identity_digest: before_subject.ast_digest.clone(),
                classification: ChangeClassification::Removed,
                changed_symbols: before_subject.symbols.clone(),
            }),
            _ => {
                subjects.push(ChangedSubject {
                    before_path: Some(before_subject.path.clone()),
                    after_path: None,
                    identity_digest: before_subject.ast_digest.clone(),
                    classification: ChangeClassification::ReviewRequired,
                    changed_symbols: before_subject.symbols.clone(),
                });
                diagnostics.push(change_diagnostic(
                    "change_scope.rename_ambiguous",
                    format!(
                        "{} has {} identical rename candidates",
                        before_subject.path,
                        rename_candidates.len()
                    ),
                    "Resolve the ambiguous rename before architecture review.",
                ));
            }
        }
    }
    for after_subject in after {
        if !matched_after.contains(after_subject.path.as_str())
            && !before_by_path.contains_key(after_subject.path.as_str())
        {
            subjects.push(ChangedSubject {
                before_path: None,
                after_path: Some(after_subject.path.clone()),
                identity_digest: after_subject.ast_digest.clone(),
                classification: ChangeClassification::Added,
                changed_symbols: after_subject.symbols.clone(),
            });
        }
    }
    subjects.sort_by(|left, right| {
        left.after_path
            .as_ref()
            .or(left.before_path.as_ref())
            .cmp(&right.after_path.as_ref().or(right.before_path.as_ref()))
    });
    ChangeScope {
        schema_version: CHANGE_SCOPE_SCHEMA_VERSION.to_string(),
        base_commit: request.base_commit.clone(),
        head_commit: request.head_commit.clone(),
        dirty_patch_digest: request.dirty_patch_digest.clone(),
        subjects,
        diagnostics,
    }
}

pub fn reconcile_touched_debt(
    scope: &ChangeScope,
    ledger: &ArchitectureDebtLedger,
    observed: &[ObservedDebt],
    today: &str,
) -> DebtReconciliation {
    let touched = scope
        .subjects
        .iter()
        .filter(|subject| subject.classification != ChangeClassification::Unchanged)
        .flat_map(|subject| {
            [
                subject.before_path.as_deref(),
                subject.after_path.as_deref(),
            ]
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let touched_observed = observed
        .iter()
        .filter(|observation| touched.contains(observation.subject_path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let touched_ledger = ArchitectureDebtLedger {
        schema_version: ledger.schema_version.clone(),
        entries: ledger
            .entries
            .iter()
            .filter(|entry| touched.contains(entry.subject_path.as_str()))
            .cloned()
            .collect(),
    };
    reconcile_debt(&touched_ledger, &touched_observed, today)
}

fn compare_subjects(
    before: &CanonicalSubject,
    after: &CanonicalSubject,
    renamed: bool,
) -> ChangedSubject {
    let classification = if renamed {
        ChangeClassification::RenameMove
    } else if before.content_digest == after.content_digest {
        ChangeClassification::Unchanged
    } else if before.ast_digest == after.ast_digest
        && before.dependency_digest == after.dependency_digest
    {
        ChangeClassification::NonSemantic
    } else {
        ChangeClassification::Semantic
    };
    let before_symbols = before.symbols.iter().collect::<BTreeSet<_>>();
    let after_symbols = after.symbols.iter().collect::<BTreeSet<_>>();
    let mut changed_symbols = before_symbols
        .symmetric_difference(&after_symbols)
        .map(|symbol| (*symbol).clone())
        .collect::<Vec<_>>();
    changed_symbols.sort();
    ChangedSubject {
        before_path: Some(before.path.clone()),
        after_path: Some(after.path.clone()),
        identity_digest: before.ast_digest.clone(),
        classification,
        changed_symbols,
    }
}

fn item_symbol(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Const(item) => Some(format!("const:{}", item.ident)),
        syn::Item::Enum(item) => Some(format!("enum:{}", item.ident)),
        syn::Item::Fn(item) => Some(format!("fn:{}", item.sig.ident)),
        syn::Item::Mod(item) => Some(format!("mod:{}", item.ident)),
        syn::Item::Static(item) => Some(format!("static:{}", item.ident)),
        syn::Item::Struct(item) => Some(format!("struct:{}", item.ident)),
        syn::Item::Trait(item) => Some(format!("trait:{}", item.ident)),
        syn::Item::Type(item) => Some(format!("type:{}", item.ident)),
        _ => None,
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn change_diagnostic(
    code: impl Into<String>,
    evidence: impl Into<String>,
    next_action: impl Into<String>,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        code: code.into(),
        source_path: None,
        domain: None,
        subject: None,
        stage: "change_scope".to_string(),
        observed_evidence: evidence.into(),
        rule_id: None,
        classification: "review_required".to_string(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_scope_classifies_format_only_as_non_semantic() {
        let before = canonical_rust_subject("src/lib.rs", "pub fn run(){ }", &[]).unwrap();
        let after = canonical_rust_subject("src/lib.rs", "pub fn run() {\n}\n", &[]).unwrap();
        let scope = compute_change_scope(&request(), &[before], &[after]);
        assert_eq!(
            scope.subjects[0].classification,
            ChangeClassification::NonSemantic
        );
    }

    #[test]
    fn change_scope_unknown_base_fails_closed() {
        let mut request = request();
        request.base_commit = "HEAD".to_string();
        let scope = compute_change_scope(&request, &[], &[]);
        assert!(scope.subjects.is_empty());
        assert_eq!(scope.diagnostics[0].code, "change_scope.unknown_base");
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
}
