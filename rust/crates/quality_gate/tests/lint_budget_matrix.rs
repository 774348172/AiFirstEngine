use quality_gate::cargo_json::{parse_cargo_json, ObservedLint};
use quality_gate::lint_ledger::{
    reconcile_lints, LintDebtEntry, LintDebtLedger, SourceSuppressionEntry,
    LINT_DEBT_LEDGER_SCHEMA_VERSION,
};
use quality_gate::report::{QualityGateReport, QUALITY_GATE_REPORT_SCHEMA_VERSION};
use quality_gate::suppression::{reconcile_suppressions, ObservedSuppression};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "quality-gate-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    root
}

fn entry(fingerprint: &str, allowed_occurrences: usize) -> LintDebtEntry {
    LintDebtEntry {
        id: format!("entry-{fingerprint}"),
        fingerprint: fingerprint.to_string(),
        lint_code: "clippy::example".to_string(),
        relative_path: "Cargo.toml".to_string(),
        anchor_hash: "sha256:anchor".to_string(),
        allowed_occurrences,
        origin: "test".to_string(),
        reason: "test debt".to_string(),
        owner: "test-owner".to_string(),
        review_by: "2099-01-01".to_string(),
    }
}

fn observed(fingerprint: &str) -> ObservedLint {
    ObservedLint {
        fingerprint: fingerprint.to_string(),
        lint_code: "clippy::example".to_string(),
        relative_path: "Cargo.toml".to_string(),
        anchor_hash: "sha256:anchor".to_string(),
        message: "message".to_string(),
    }
}

fn ledger(entries: Vec<LintDebtEntry>) -> LintDebtLedger {
    LintDebtLedger {
        schema_version: LINT_DEBT_LEDGER_SCHEMA_VERSION.to_string(),
        toolchain: "1.96.0".to_string(),
        generated_from: "test".to_string(),
        entries,
        source_suppressions: Vec::new(),
    }
}

#[test]
fn new_replaced_grown_resolved_and_expired_debt_fail_closed() {
    let root = temp_root("lint-matrix");
    let today = "2026-07-12";

    let new = reconcile_lints(
        &root,
        "1.96.0",
        &ledger(Vec::new()),
        &[observed("new")],
        today,
    );
    assert_eq!(new.summary.new_entries, 1);

    let replaced = reconcile_lints(
        &root,
        "1.96.0",
        &ledger(vec![entry("old", 1)]),
        &[observed("new")],
        today,
    );
    assert_eq!(replaced.summary.new_entries, 1);
    assert_eq!(replaced.summary.resolved_entries, 1);

    let grown = reconcile_lints(
        &root,
        "1.96.0",
        &ledger(vec![entry("same", 1)]),
        &[observed("same"), observed("same")],
        today,
    );
    assert!(grown
        .diagnostics
        .iter()
        .any(|item| item.code == "quality_gate.lint_occurrence_exceeded"));

    let resolved = reconcile_lints(&root, "1.96.0", &ledger(vec![entry("old", 1)]), &[], today);
    assert_eq!(resolved.summary.resolved_entries, 1);

    let mut expired_entry = entry("same", 1);
    expired_entry.review_by = "2020-01-01".to_string();
    let expired = reconcile_lints(
        &root,
        "1.96.0",
        &ledger(vec![expired_entry]),
        &[observed("same")],
        today,
    );
    assert_eq!(expired.summary.stale_entries, 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fingerprint_ignores_line_motion_but_detects_anchor_change() {
    fn warning(line: u64, anchor: &str) -> Vec<u8> {
        serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "warning",
                "message": "same message",
                "code": {"code": "clippy::example"},
                "spans": [{
                    "is_primary": true,
                    "file_name": "crates/example/src/lib.rs",
                    "line_start": line,
                    "text": [{"text": anchor}]
                }]
            }
        })
        .to_string()
        .into_bytes()
    }

    let first = parse_cargo_json(&warning(1, "let value = 1;")).unwrap();
    let moved = parse_cargo_json(&warning(500, "let   value = 1;")).unwrap();
    let changed = parse_cargo_json(&warning(1, "let value = 2;")).unwrap();
    assert_eq!(first[0].fingerprint, moved[0].fingerprint);
    assert_ne!(first[0].fingerprint, changed[0].fingerprint);
    assert!(parse_cargo_json(b"{truncated").is_err());
}

#[test]
fn registered_suppression_passes_and_unregistered_suppression_fails() {
    let root = temp_root("suppression-matrix");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "fn main() {}\n").unwrap();
    let suppression = ObservedSuppression {
        lint: "dead_code".to_string(),
        relative_path: "src/lib.rs".to_string(),
        anchor_hash: "sha256:anchor".to_string(),
    };
    let mut registered_ledger = ledger(Vec::new());
    registered_ledger.source_suppressions = vec![SourceSuppressionEntry {
        id: "suppression-1".to_string(),
        lint: suppression.lint.clone(),
        relative_path: suppression.relative_path.clone(),
        anchor_hash: suppression.anchor_hash.clone(),
        allowed_occurrences: 1,
        reason: "test".to_string(),
        owner: "test-owner".to_string(),
        review_by: "2099-01-01".to_string(),
    }];
    assert!(reconcile_suppressions(
        &root,
        &registered_ledger,
        std::slice::from_ref(&suppression),
        "2026-07-12"
    )
    .passed());
    assert!(
        !reconcile_suppressions(&root, &ledger(Vec::new()), &[suppression], "2026-07-12").passed()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn report_schema_roundtrips_without_absolute_paths() {
    let report = QualityGateReport {
        workspace_state: "dirty".to_string(),
        ..QualityGateReport::default()
    };
    let json = serde_json::to_string(&report).unwrap();
    let decoded: QualityGateReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.schema_version, QUALITY_GATE_REPORT_SCHEMA_VERSION);
    assert!(!json.contains("C:\\Users"));
    assert!(!json.contains("/home/"));
}
