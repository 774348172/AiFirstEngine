use editor_core::RepairScopeValidation;
use runtime_cli::{BoundedChildProcessExitReason, BoundedChildProcessResult};
use serde::{Deserialize, Serialize};

pub const CRITICAL_CORRECTNESS_SAFETY_GATE_REPORT_SCHEMA_VERSION: &str =
    "critical-correctness-safety-gate-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticalCorrectnessSafetyStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairScopeSummary {
    pub status: CriticalCorrectnessSafetyStatus,
    pub accepted: bool,
    pub initial_operation_count: Option<usize>,
    pub repaired_operation_count: usize,
    pub changed_slots: Vec<usize>,
    pub rejection_code: Option<String>,
}

impl RepairScopeSummary {
    pub fn from_validation(validation: &RepairScopeValidation) -> Self {
        let accepted = validation.accepted();
        Self {
            status: if accepted {
                CriticalCorrectnessSafetyStatus::Passed
            } else {
                CriticalCorrectnessSafetyStatus::Failed
            },
            accepted,
            initial_operation_count: validation.initial_operation_count,
            repaired_operation_count: validation.repaired_operation_count,
            changed_slots: validation.changed_slots.clone(),
            rejection_code: validation.rejection_code.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessLifecycleSummary {
    pub status: CriticalCorrectnessSafetyStatus,
    pub exit_reason: String,
    pub exit_code: Option<i32>,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup_errors: Vec<String>,
}

impl ProcessLifecycleSummary {
    pub fn from_result(result: &BoundedChildProcessResult) -> Self {
        let cleanup_errors = [
            result.spawn_error.clone(),
            result.kill_error.clone(),
            result.wait_error.clone(),
            result.reader_join_error.clone(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let passed = result.exit_reason == BoundedChildProcessExitReason::Completed
            && result.exit_code == Some(0)
            && cleanup_errors.is_empty();
        Self {
            status: if passed {
                CriticalCorrectnessSafetyStatus::Passed
            } else {
                CriticalCorrectnessSafetyStatus::Failed
            },
            exit_reason: match result.exit_reason {
                BoundedChildProcessExitReason::Completed => "completed",
                BoundedChildProcessExitReason::Failed => "failed",
                BoundedChildProcessExitReason::Cancelled => "cancelled",
                BoundedChildProcessExitReason::Timeout => "timeout",
                BoundedChildProcessExitReason::WaitFailed => "wait_failed",
                BoundedChildProcessExitReason::SpawnFailed => "spawn_failed",
            }
            .to_string(),
            exit_code: result.exit_code,
            stdout_total_bytes: result.stdout_total_bytes,
            stderr_total_bytes: result.stderr_total_bytes,
            stdout_truncated: result.stdout_truncated,
            stderr_truncated: result.stderr_truncated,
            cleanup_errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishLockSummary {
    pub status: CriticalCorrectnessSafetyStatus,
    pub process_count: usize,
    pub rounds: usize,
    pub successful_publishes: usize,
    pub maximum_active_publishers: usize,
    pub stable_lock_path_present: bool,
    pub final_payload_hash_matched: bool,
}

impl PublishLockSummary {
    pub fn from_handoff_evidence(
        process_count: usize,
        rounds: usize,
        successful_publishes: usize,
        maximum_active_publishers: usize,
        stable_lock_path_present: bool,
        final_payload_hash_matched: bool,
    ) -> Self {
        let passed = process_count >= 3
            && rounds > 1
            && successful_publishes == process_count * rounds
            && maximum_active_publishers <= 1
            && stable_lock_path_present
            && final_payload_hash_matched;
        Self {
            status: if passed {
                CriticalCorrectnessSafetyStatus::Passed
            } else {
                CriticalCorrectnessSafetyStatus::Failed
            },
            process_count,
            rounds,
            successful_publishes,
            maximum_active_publishers,
            stable_lock_path_present,
            final_payload_hash_matched,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeContractSummary {
    pub status: CriticalCorrectnessSafetyStatus,
    pub mutation_count: usize,
    pub resource_specific_rejections: usize,
    pub hash_consistent_mutations: usize,
    pub normal_package_verified: bool,
}

impl PeContractSummary {
    pub fn from_mutation_evidence(
        mutation_count: usize,
        resource_specific_rejections: usize,
        hash_consistent_mutations: usize,
        normal_package_verified: bool,
    ) -> Self {
        let passed = mutation_count > 0
            && resource_specific_rejections == mutation_count
            && hash_consistent_mutations == mutation_count
            && normal_package_verified;
        Self {
            status: if passed {
                CriticalCorrectnessSafetyStatus::Passed
            } else {
                CriticalCorrectnessSafetyStatus::Failed
            },
            mutation_count,
            resource_specific_rejections,
            hash_consistent_mutations,
            normal_package_verified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriticalCorrectnessSafetyDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriticalCorrectnessSafetyGateReport {
    pub schema_version: String,
    pub status: CriticalCorrectnessSafetyStatus,
    pub repair_scope_summary: RepairScopeSummary,
    pub process_lifecycle_summary: ProcessLifecycleSummary,
    pub publish_lock_summary: PublishLockSummary,
    pub pe_contract_summary: PeContractSummary,
    pub diagnostics: Vec<CriticalCorrectnessSafetyDiagnostic>,
    pub next_actions: Vec<String>,
}

impl CriticalCorrectnessSafetyGateReport {
    pub fn aggregate(
        repair_scope_summary: RepairScopeSummary,
        process_lifecycle_summary: ProcessLifecycleSummary,
        publish_lock_summary: PublishLockSummary,
        pe_contract_summary: PeContractSummary,
    ) -> Self {
        let checks = [
            ("repair_scope_failed", repair_scope_summary.status),
            ("process_lifecycle_failed", process_lifecycle_summary.status),
            ("publish_lock_failed", publish_lock_summary.status),
            ("pe_contract_failed", pe_contract_summary.status),
        ];
        let diagnostics = checks
            .into_iter()
            .filter(|(_, status)| *status == CriticalCorrectnessSafetyStatus::Failed)
            .map(|(code, _)| CriticalCorrectnessSafetyDiagnostic {
                severity: "error".to_string(),
                code: code.to_string(),
                message: format!("{code} evidence did not satisfy the 239 acceptance contract"),
            })
            .collect::<Vec<_>>();
        let status = if diagnostics.is_empty() {
            CriticalCorrectnessSafetyStatus::Passed
        } else {
            CriticalCorrectnessSafetyStatus::Failed
        };
        let next_actions = diagnostics
            .iter()
            .map(|diagnostic| format!("repair_{}", diagnostic.code))
            .collect();
        Self {
            schema_version: CRITICAL_CORRECTNESS_SAFETY_GATE_REPORT_SCHEMA_VERSION.to_string(),
            status,
            repair_scope_summary,
            process_lifecycle_summary,
            publish_lock_summary,
            pe_contract_summary,
            diagnostics,
            next_actions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn critical_correctness_safety_report_aggregates_existing_gate_evidence() {
        let repair = RepairScopeSummary {
            status: CriticalCorrectnessSafetyStatus::Passed,
            accepted: true,
            initial_operation_count: Some(1),
            repaired_operation_count: 1,
            changed_slots: vec![0],
            rejection_code: None,
        };
        let process = ProcessLifecycleSummary::from_result(&BoundedChildProcessResult {
            process_id: Some(42),
            exit_reason: BoundedChildProcessExitReason::Completed,
            exit_code: Some(0),
            elapsed_ms: 10,
            stdout_summary: "bounded".to_string(),
            stderr_summary: String::new(),
            stdout_total_bytes: 1_048_833,
            stderr_total_bytes: 1_048_833,
            stdout_truncated: true,
            stderr_truncated: true,
            spawn_error: None,
            kill_error: None,
            wait_error: None,
            reader_join_error: None,
            ownership: runtime_cli::BoundedProcessOwnershipEvidence::default(),
            priority: runtime_cli::BoundedChildProcessPriorityEvidence::default(),
        });
        let publish = PublishLockSummary::from_handoff_evidence(3, 6, 18, 1, true, true);
        let pe = PeContractSummary::from_mutation_evidence(11, 11, 11, true);

        let report = CriticalCorrectnessSafetyGateReport::aggregate(repair, process, publish, pe);
        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(report.status, CriticalCorrectnessSafetyStatus::Passed);
        assert!(report.diagnostics.is_empty());
        assert!(report.next_actions.is_empty());
        assert_eq!(
            value["schemaVersion"],
            CRITICAL_CORRECTNESS_SAFETY_GATE_REPORT_SCHEMA_VERSION
        );
        for field in [
            "repairScopeSummary",
            "processLifecycleSummary",
            "publishLockSummary",
            "peContractSummary",
            "diagnostics",
            "nextActions",
        ] {
            assert!(
                value.get(field).is_some(),
                "missing aggregate field {field}"
            );
        }
    }

    #[test]
    fn critical_correctness_safety_report_fails_closed_on_missing_acceptance() {
        let report = CriticalCorrectnessSafetyGateReport::aggregate(
            RepairScopeSummary {
                status: CriticalCorrectnessSafetyStatus::Failed,
                accepted: false,
                initial_operation_count: Some(1),
                repaired_operation_count: 2,
                changed_slots: vec![0, 1],
                rejection_code: Some("repair_scope_operation_count_expanded".to_string()),
            },
            ProcessLifecycleSummary {
                status: CriticalCorrectnessSafetyStatus::Passed,
                exit_reason: "completed".to_string(),
                exit_code: Some(0),
                stdout_total_bytes: 0,
                stderr_total_bytes: 0,
                stdout_truncated: false,
                stderr_truncated: false,
                cleanup_errors: Vec::new(),
            },
            PublishLockSummary::from_handoff_evidence(3, 2, 6, 1, true, true),
            PeContractSummary::from_mutation_evidence(11, 11, 11, true),
        );

        assert_eq!(report.status, CriticalCorrectnessSafetyStatus::Failed);
        assert_eq!(report.diagnostics[0].code, "repair_scope_failed");
        assert_eq!(report.next_actions, vec!["repair_repair_scope_failed"]);
    }
}
