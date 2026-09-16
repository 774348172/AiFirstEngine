pub mod architecture_artifact;
mod architecture_bootstrap;
pub mod architecture_coverage;
pub mod architecture_debt;
pub mod architecture_inventory;
pub mod architecture_policy;
pub mod architecture_review;
pub mod cargo_json;
pub mod change_scope;
pub mod command;
pub mod construction_validation;
pub mod lint_ledger;
mod local_ci;
mod proposal;
pub mod report;
mod runner;
pub mod suppression;
pub mod toolchain;
pub mod validation_catalog;
pub mod validation_evidence;
mod workspace;

pub use command::{
    QualityCommandExecutor, ScriptedQualityCommandExecutor, SystemQualityCommandExecutor,
};
pub use construction_validation::{
    prepare_validation_plan_files, resolve_validation_plan_output, AuthorizationCeiling,
    ChangeIdentity, ChangedSubjectFact, ConstructionValidationModule, EliminatedDuplicate, PlanRef,
    PlannedStage, PrepareReport, PrepareRequest, PrepareStatus, ProofObligation, ReusedEvidence,
    ValidationClaim, ValidationDiagnostic,
};
pub use local_ci::{LocalCiRequest, LocalCiRunner};
pub use report::{LocalCiRunReport, QualityGateReport, QualityProfile, ReportMode};
pub use runner::{QualityGateRequest, QualityGateRunner};
pub use validation_catalog::{
    catalog_digest, parse_validation_catalog, AuthorizationRequirement, CostClass,
    EnvironmentIdentity, EnvironmentRequirement, EvidenceIdentityKind, ExternalEffect,
    StaticValidationCatalogSource, ValidationCatalog, ValidationCatalogSource,
    VerifierCatalogEntry,
};
pub use validation_evidence::{
    EmptyEvidenceStore, EvidenceRecord, EvidenceStore, InMemoryEvidenceStore,
};

use std::fs;
use std::path::PathBuf;

pub fn run_cli(args: impl IntoIterator<Item = String>) -> i32 {
    let args = args.into_iter().collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        eprintln!("usage: quality_gate <verify|local-ci|propose-ledger|validation-plan> [options]");
        return 2;
    };
    let workspace_root = argument_value(&args, "--workspace-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    match command {
        "verify" => {
            let executor = SystemQualityCommandExecutor;
            let report_mode = match argument_value(&args, "--report-mode").as_deref() {
                None | Some("summary") => ReportMode::Summary,
                Some("trace") => ReportMode::Trace,
                Some(value) => {
                    eprintln!("unsupported --report-mode {value:?}; use summary or trace");
                    return 2;
                }
            };
            let mut request = QualityGateRequest::new(workspace_root);
            request.report_mode = report_mode;
            request.fail_fast = has_flag(&args, "--fail-fast");
            request.report_output = argument_value(&args, "--report-output").map(PathBuf::from);
            request.profile = match argument_value(&args, "--profile").as_deref() {
                None | Some("fast") => QualityProfile::Fast,
                Some("engine-strict") => QualityProfile::EngineStrict,
                Some("project-advisory") => QualityProfile::ProjectAdvisory,
                Some("project-strict") => QualityProfile::ProjectStrict,
                Some(value) => {
                    eprintln!("unsupported --profile {value:?}");
                    return 2;
                }
            };
            request.base_commit = argument_value(&args, "--base-commit");
            request.head_commit = argument_value(&args, "--head-commit");
            request.dirty_patch_digest = argument_value(&args, "--dirty-patch-digest");
            request.architecture_artifact =
                argument_value(&args, "--architecture-artifact").map(PathBuf::from);
            if let Some(path) = argument_value(&args, "--artifact-expectation") {
                let source = match fs::read(&path) {
                    Ok(source) => source,
                    Err(error) => {
                        eprintln!("failed to read artifact expectation {path:?}: {error}");
                        return 2;
                    }
                };
                request.artifact_expectation = match serde_json::from_slice(&source) {
                    Ok(expectation) => Some(expectation),
                    Err(error) => {
                        eprintln!("invalid artifact expectation {path:?}: {error}");
                        return 2;
                    }
                };
            }
            let report = QualityGateRunner::new(executor).verify(request);
            match serde_json::to_string_pretty(&report) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to serialize quality gate report: {error}");
                    return 1;
                }
            }
            i32::from(!report.passed)
        }
        "local-ci" => {
            let executor = SystemQualityCommandExecutor;
            let Some(revision) = argument_value(&args, "--commit") else {
                eprintln!("local-ci requires --commit <revision>");
                return 2;
            };
            let report =
                LocalCiRunner::new(executor).run(LocalCiRequest::new(workspace_root, revision));
            match serde_json::to_string_pretty(&report) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to serialize local CI report: {error}");
                    return 1;
                }
            }
            i32::from(!report.passed)
        }
        "propose-ledger" => {
            match proposal::propose_ledger(&SystemQualityCommandExecutor, &workspace_root) {
                Ok(proposal) => {
                    println!(
                        "candidate ledger: entries={}, warning_occurrences={}, suppressions={}",
                        proposal.entry_count,
                        proposal.warning_occurrences,
                        proposal.suppression_count
                    );
                    println!(
                    "candidate files: target/quality-gate/candidate/{} and target/quality-gate/candidate/{}",
                    proposal
                        .ledger_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("lint-debt-ledger.v1.json"),
                    proposal
                        .diff_report_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("lint-debt-ledger-diff.v1.json")
                );
                    0
                }
                Err(error) => {
                    eprintln!("failed to propose lint ledger: {error}");
                    1
                }
            }
        }
        "propose-architecture-baseline" => {
            match architecture_bootstrap::propose_architecture_baseline(
                &SystemQualityCommandExecutor,
                &workspace_root,
            ) {
                Ok(report) => {
                    println!(
                        "architecture baseline candidate: inventory_files={}, debt_entries={}, coverage_entries={}",
                        report.inventory_files, report.debt_entries, report.coverage_entries
                    );
                    println!("candidate root: target/quality-gate/candidate");
                    0
                }
                Err(error) => {
                    eprintln!("failed to propose architecture baseline: {error}");
                    1
                }
            }
        }
        "validation-plan" => {
            if has_flag(&args, "--execute") || has_flag(&args, "--apply") {
                eprintln!("validation-plan is plan-only and rejects execution flags");
                return 2;
            }
            let Some(request_path) = argument_value(&args, "--request").map(PathBuf::from) else {
                eprintln!("validation-plan requires --request <request.json>");
                return 2;
            };
            let Some(catalog_path) = argument_value(&args, "--catalog").map(PathBuf::from) else {
                eprintln!("validation-plan requires --catalog <catalog.json>");
                return 2;
            };
            let Some(output_path) = argument_value(&args, "--report-output").map(PathBuf::from)
            else {
                eprintln!("validation-plan requires --report-output <target path>");
                return 2;
            };
            match prepare_validation_plan_files(
                &workspace_root,
                &request_path,
                &catalog_path,
                &output_path,
            ) {
                Ok(report) => {
                    match serde_json::to_string_pretty(&report) {
                        Ok(json) => println!("{json}"),
                        Err(error) => {
                            eprintln!("failed to serialize validation plan: {error}");
                            return 1;
                        }
                    }
                    i32::from(report.status != PrepareStatus::Ready)
                }
                Err(diagnostic) => {
                    match serde_json::to_string_pretty(&diagnostic) {
                        Ok(json) => eprintln!("{json}"),
                        Err(error) => eprintln!("validation-plan failed: {error}"),
                    }
                    1
                }
            }
        }
        other => {
            eprintln!("unsupported quality gate command {other:?}");
            2
        }
    }
}

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|argument| argument == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static VALIDATION_PLAN_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn argument_value_reads_named_option() {
        let args = vec![
            "verify".to_string(),
            "--report-mode".to_string(),
            "trace".to_string(),
        ];
        assert_eq!(
            argument_value(&args, "--report-mode").as_deref(),
            Some("trace")
        );
    }

    #[test]
    fn has_flag_reads_boolean_switch() {
        let args = vec!["verify".to_string(), "--fail-fast".to_string()];
        assert!(has_flag(&args, "--fail-fast"));
        assert!(!has_flag(&args, "--other"));
    }

    fn validation_plan_workspace(label: &str) -> PathBuf {
        let sequence = VALIDATION_PLAN_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "aife-quality-gate-validation-plan-{}-{label}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn validation_plan_request() -> &'static str {
        r#"{
  "schemaVersion":"construction-validation.prepare-request.v2",
  "requestId":"cli-plan",
  "claim":"Development",
  "change":{
    "baseCommit":null,
    "headCommit":null,
    "dirtyPatchDigest":"sha256:patch",
    "changedSubjects":[{"path":"crates/quality_gate/src/lib.rs","ownerDomain":"quality_gate"}],
    "identities":{"product_source":"sha256:source","verification_harness":"sha256:harness"}
  },
  "declaredOwners":[],
  "declaredConsumers":[],
  "requiredCapabilities":[],
  "environment":{"platform":"windows-x86_64","profile":"debug","features":[],"composition":"source"},
  "authorizationCeiling":{"localCi":false,"realEditor":false,"productionReplacement":false,"realConfigurationMutation":false},
  "timeBudgetSeconds":300
}"#
    }

    fn validation_plan_catalog() -> &'static str {
        r#"{
  "schemaVersion":"construction-validation.catalog.v1",
  "verifiers":[{
    "id":"quality_gate.owner",
    "ownerDomains":["quality_gate"],
    "consumerDomains":["quality_gate_cli"],
    "proves":["development.owner"],
    "producerId":"quality_gate.owner_tests",
    "environment":{},
    "subsumes":[],
    "costClass":"low",
    "defaultTimeoutSeconds":60,
    "historicalDurationSeconds":5,
    "cleanupReserveSeconds":0,
    "externalEffects":[],
    "requiredAuthorization":[],
    "consumedIdentityKinds":["product_source","verification_harness"]
  }]
}"#
    }

    fn write_validation_plan_inputs(root: &std::path::Path) -> (PathBuf, PathBuf) {
        let request = root.join("request.json");
        let catalog = root.join("catalog.json");
        fs::write(&request, validation_plan_request()).unwrap();
        fs::write(&catalog, validation_plan_catalog()).unwrap();
        (request, catalog)
    }

    #[test]
    fn validation_plan_cli_writes_run_owned_report_without_executor() {
        let root = validation_plan_workspace("valid");
        let (request, catalog) = write_validation_plan_inputs(&root);
        let executor = ScriptedQualityCommandExecutor::default();
        let output = "target/quality-gate/validation-plans/run-a/prepare-report.v2.json";
        let exit = run_cli([
            "validation-plan".to_string(),
            "--workspace-root".to_string(),
            root.display().to_string(),
            "--request".to_string(),
            request.display().to_string(),
            "--catalog".to_string(),
            catalog.display().to_string(),
            "--report-output".to_string(),
            output.to_string(),
        ]);
        assert_eq!(exit, 0);
        assert!(root.join(output).is_file());
        assert!(executor.observed().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validation_plan_cli_rejects_escape_and_execution_flags() {
        let root = validation_plan_workspace("escape");
        let (request, catalog) = write_validation_plan_inputs(&root);
        let base = vec![
            "validation-plan".to_string(),
            "--workspace-root".to_string(),
            root.display().to_string(),
            "--request".to_string(),
            request.display().to_string(),
            "--catalog".to_string(),
            catalog.display().to_string(),
            "--report-output".to_string(),
            "../escape.json".to_string(),
        ];
        assert_eq!(run_cli(base.clone()), 1);
        let mut execute = base;
        execute.push("--execute".to_string());
        assert_eq!(run_cli(execute), 2);
        assert!(!root.parent().unwrap().join("escape.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validation_plan_cli_does_not_overwrite_different_content() {
        let root = validation_plan_workspace("existing");
        let (request, catalog) = write_validation_plan_inputs(&root);
        let relative =
            PathBuf::from("target/quality-gate/validation-plans/run-a/prepare-report.v2.json");
        let output = root.join(&relative);
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"different").unwrap();
        assert!(prepare_validation_plan_files(&root, &request, &catalog, &relative).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"different");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validation_plan_cli_malformed_request_writes_nothing() {
        let root = validation_plan_workspace("malformed");
        let (request, catalog) = write_validation_plan_inputs(&root);
        fs::write(&request, b"{bad-json}").unwrap();
        let relative =
            PathBuf::from("target/quality-gate/validation-plans/run-a/prepare-report.v2.json");
        assert!(prepare_validation_plan_files(&root, &request, &catalog, &relative).is_err());
        assert!(!root.join(relative).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn validation_plan_cli_rejects_reparse_output_component_when_supported() {
        let root = validation_plan_workspace("reparse");
        let outside = validation_plan_workspace("reparse-outside");
        let plan_root = root.join("target/quality-gate/validation-plans");
        fs::create_dir_all(plan_root.parent().unwrap()).unwrap();
        if std::os::windows::fs::symlink_dir(&outside, &plan_root).is_ok() {
            let requested =
                PathBuf::from("target/quality-gate/validation-plans/run-a/prepare-report.v2.json");
            assert!(resolve_validation_plan_output(&root, &requested).is_err());
            fs::remove_dir(&plan_root).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
