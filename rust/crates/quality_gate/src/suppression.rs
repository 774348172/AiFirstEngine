use crate::cargo_json::{normalize_path, normalize_whitespace, sha256_hex};
use crate::lint_ledger::{date_is_before, LintDebtLedger, SourceSuppressionEntry};
use crate::report::{LintGateSummary, QualityDiagnostic};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, Meta, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedSuppression {
    pub lint: String,
    pub relative_path: String,
    pub anchor_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppressionReconciliation {
    pub diagnostics: Vec<QualityDiagnostic>,
    pub observed_count: usize,
    pub new_count: usize,
}

impl SuppressionReconciliation {
    pub fn apply_to_summary(&self, summary: &mut LintGateSummary) {
        summary.suppression_entries = self.observed_count;
        summary.new_suppressions = self.new_count;
    }

    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn scan_workspace_suppressions(
    workspace_root: &Path,
) -> Result<Vec<ObservedSuppression>, String> {
    let mut files = Vec::new();
    for directory in ["crates", "project_modules", "project_players"] {
        collect_rust_files(&workspace_root.join(directory), &mut files)?;
    }
    if let Some(repository_root) = workspace_root.parent() {
        collect_rust_files(&repository_root.join("samples"), &mut files)?;
    }
    files.sort();
    let mut observed = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let syntax =
            syn::parse_file(&source).map_err(|error| format!("{}: {error}", path.display()))?;
        let relative_path = relative_source_path(workspace_root, &path)?;
        let mut collector = SuppressionCollector {
            relative_path,
            observed: Vec::new(),
        };
        collector.visit_file(&syntax);
        observed.extend(collector.observed);
    }
    Ok(observed)
}

fn relative_source_path(workspace_root: &Path, path: &Path) -> Result<String, String> {
    if let Ok(relative) = path.strip_prefix(workspace_root) {
        return Ok(normalize_path(relative.to_string_lossy().as_ref()));
    }
    let repository_root = workspace_root
        .parent()
        .ok_or_else(|| format!("{} is outside the workspace", path.display()))?;
    let relative = path
        .strip_prefix(repository_root)
        .map_err(|_| format!("{} is outside the repository", path.display()))?;
    Ok(format!(
        "../{}",
        normalize_path(relative.to_string_lossy().as_ref())
    ))
}

pub fn reconcile_suppressions(
    workspace_root: &Path,
    ledger: &LintDebtLedger,
    observed: &[ObservedSuppression],
    today: &str,
) -> SuppressionReconciliation {
    let mut actual = BTreeMap::<String, usize>::new();
    for item in observed {
        *actual
            .entry(suppression_key(
                &item.relative_path,
                &item.lint,
                &item.anchor_hash,
            ))
            .or_default() += 1;
    }
    let mut known = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for entry in &ledger.source_suppressions {
        let key = suppression_entry_key(entry);
        known.insert(key.clone());
        let path = workspace_root.join(
            entry
                .relative_path
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let expired = date_is_before(&entry.review_by, today);
        if !path.is_file() || expired {
            diagnostics.push(diagnostic(
                "quality_gate.suppression_stale",
                format!("suppression entry {} is stale or expired", entry.id),
                "Remove the suppression or renew its reviewed exception.",
            ));
        }
        match actual.get(&key).copied() {
            Some(count) if count <= entry.allowed_occurrences => {}
            Some(count) => diagnostics.push(diagnostic(
                "quality_gate.suppression_unregistered",
                format!(
                    "suppression {} occurs {count} times; ledger allows {}",
                    entry.id, entry.allowed_occurrences
                ),
                "Remove the new suppression or submit a reviewed exception.",
            )),
            None => diagnostics.push(diagnostic(
                "quality_gate.suppression_stale",
                format!("suppression entry {} no longer matches source", entry.id),
                "Remove the stale source_suppressions entry.",
            )),
        }
    }
    let mut new_count = 0;
    for (key, count) in &actual {
        if !known.contains(key) {
            new_count += count;
            diagnostics.push(diagnostic(
                "quality_gate.suppression_unregistered",
                format!("unregistered source suppression {key}"),
                "Remove it or add a reviewed source_suppressions entry.",
            ));
        }
    }
    SuppressionReconciliation {
        diagnostics,
        observed_count: observed.len(),
        new_count,
    }
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            if !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("target" | ".git")
            ) {
                collect_rust_files(&path, files)?;
            }
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            files.push(path);
        }
    }
    Ok(())
}

struct SuppressionCollector {
    relative_path: String,
    observed: Vec<ObservedSuppression>,
}

impl SuppressionCollector {
    fn collect_attribute(&mut self, attribute: &Attribute) {
        if attribute.path().is_ident("allow") || attribute.path().is_ident("expect") {
            let kind = if attribute.path().is_ident("allow") {
                "allow"
            } else {
                "expect"
            };
            if let Ok(paths) =
                attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            {
                for path in paths {
                    let lint = path_to_string(&path);
                    self.push(kind, &lint, &format!("#[{kind}({lint})]"));
                }
            }
            return;
        }
        if !attribute.path().is_ident("cfg_attr") {
            return;
        }
        let Meta::List(attribute_list) = &attribute.meta else {
            return;
        };
        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        let Ok(items) = parser.parse2(attribute_list.tokens.clone()) else {
            return;
        };
        let condition = items
            .first()
            .map(ToTokens::to_token_stream)
            .map(|tokens| normalize_whitespace(&tokens.to_string()))
            .unwrap_or_default();
        for meta in items.iter().skip(1) {
            let Meta::List(list) = meta else {
                continue;
            };
            let kind = if list.path.is_ident("allow") {
                "allow"
            } else if list.path.is_ident("expect") {
                "expect"
            } else {
                continue;
            };
            if let Ok(paths) =
                Punctuated::<syn::Path, Token![,]>::parse_terminated.parse2(list.tokens.clone())
            {
                for path in paths {
                    let lint = path_to_string(&path);
                    self.push(
                        kind,
                        &lint,
                        &format!("#[cfg_attr({condition},{kind}({lint}))]"),
                    );
                }
            }
        }
    }

    fn push(&mut self, _kind: &str, lint: &str, anchor: &str) {
        self.observed.push(ObservedSuppression {
            lint: lint.to_string(),
            relative_path: self.relative_path.clone(),
            anchor_hash: format!("sha256:{}", sha256_hex(anchor.as_bytes())),
        });
    }
}

impl<'ast> Visit<'ast> for SuppressionCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.collect_attribute(attribute);
        syn::visit::visit_attribute(self, attribute);
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn suppression_entry_key(entry: &SourceSuppressionEntry) -> String {
    suppression_key(&entry.relative_path, &entry.lint, &entry.anchor_hash)
}

fn suppression_key(path: &str, lint: &str, anchor_hash: &str) -> String {
    format!("{path}|{lint}|{anchor_hash}")
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

    #[test]
    fn syn_scanner_finds_allow_expect_and_cfg_attr() {
        let syntax = syn::parse_file(
            r#"
                #![allow(dead_code)]
                #[expect(clippy::large_enum_variant)]
                #[cfg_attr(test, allow(clippy::too_many_arguments))]
                fn example() {}
            "#,
        )
        .unwrap();
        let mut collector = SuppressionCollector {
            relative_path: "src/lib.rs".to_string(),
            observed: Vec::new(),
        };
        collector.visit_file(&syntax);
        let lints: BTreeSet<_> = collector
            .observed
            .iter()
            .map(|item| item.lint.as_str())
            .collect();
        assert_eq!(
            lints,
            BTreeSet::from([
                "dead_code",
                "clippy::large_enum_variant",
                "clippy::too_many_arguments"
            ])
        );
    }

    #[test]
    fn unregistered_suppression_fails() {
        let ledger = LintDebtLedger {
            schema_version: "lint-debt-ledger.v1".to_string(),
            toolchain: "1.96.0".to_string(),
            generated_from: "test".to_string(),
            entries: Vec::new(),
            source_suppressions: Vec::new(),
        };
        let result = reconcile_suppressions(
            Path::new("."),
            &ledger,
            &[ObservedSuppression {
                lint: "dead_code".to_string(),
                relative_path: "src/lib.rs".to_string(),
                anchor_hash: "sha256:anchor".to_string(),
            }],
            "2026-07-12",
        );
        assert!(!result.passed());
        assert_eq!(result.new_count, 1);
    }

    #[test]
    fn repository_sample_path_is_workspace_relative_without_absolute_prefix() {
        let workspace = Path::new("repository/rust");
        let sample = Path::new("repository/samples/example/RuntimeModule/src/lib.rs");
        assert_eq!(
            relative_source_path(workspace, sample).unwrap(),
            "../samples/example/RuntimeModule/src/lib.rs"
        );
    }
}
