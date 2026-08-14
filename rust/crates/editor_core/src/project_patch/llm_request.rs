use serde::{Deserialize, Serialize};

use super::{
    CancelSource, CredentialOwnerStatus, LlmLifecycleState, LlmLocalExecutionStatus,
    LlmPatchSourceStatus, LlmRemoteExecutionStatus, LlmStructuredOutputMode, LlmTaskJoinStatus,
    LlmTerminalStatus, LlmTransportCancelCapability, RepairScopeValidation,
    RepairScopeValidationStatus,
};

pub const LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION: &str = "llm-patch-request-report.v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmPatchReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPatchAttemptSummary {
    pub attempt_kind: String,
    pub attempt_index: u8,
    pub status: LlmPatchSourceStatus,
    pub latency_ms: u64,
    pub http_status_class: Option<String>,
    pub transport_attempt_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPatchRepairScopeEvidence {
    pub status: RepairScopeValidationStatus,
    pub initial_operation_count: Option<usize>,
    pub repaired_operation_count: usize,
    pub changed_slots: Vec<usize>,
    pub diagnostic_codes: Vec<String>,
    pub rejection_code: Option<String>,
}

impl From<&RepairScopeValidation> for LlmPatchRepairScopeEvidence {
    fn from(validation: &RepairScopeValidation) -> Self {
        Self {
            status: validation.status,
            initial_operation_count: validation.initial_operation_count,
            repaired_operation_count: validation.repaired_operation_count,
            changed_slots: validation.changed_slots.clone(),
            diagnostic_codes: validation.diagnostic_codes.clone(),
            rejection_code: validation.rejection_code.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPatchRequestReport {
    pub schema_version: String,
    pub report_level: LlmPatchReportLevel,
    pub request_id: String,
    pub provider_id: String,
    pub model: String,
    pub structured_output_mode: LlmStructuredOutputMode,
    pub final_status: String,
    pub repair_attempt_count: u8,
    pub repair_scope: Option<LlmPatchRepairScopeEvidence>,
    pub candidate_hash: Option<String>,
    pub diagnostic_codes: Vec<String>,
    pub context_hash: Option<String>,
    pub schema_hash: Option<String>,
    pub context_stale: bool,
    pub cancelled: bool,
    pub attempts: Vec<LlmPatchAttemptSummary>,
    pub lifecycle_state: LlmLifecycleState,
    pub terminal_status: Option<LlmTerminalStatus>,
    pub cancel_requested: bool,
    pub cancel_source: CancelSource,
    pub transport_cancel_capability: LlmTransportCancelCapability,
    pub transport_abort_requested: bool,
    pub transport_abort_observed: bool,
    pub task_join_status: LlmTaskJoinStatus,
    pub credential_owner_status: CredentialOwnerStatus,
    pub local_execution_status: LlmLocalExecutionStatus,
    pub remote_execution_status: LlmRemoteExecutionStatus,
    pub cancel_latency_ms: Option<u64>,
    pub shutdown_latency_ms: Option<u64>,
}

impl LlmPatchRequestReport {
    pub fn started(
        level: LlmPatchReportLevel,
        request_id: impl Into<String>,
        provider_id: impl Into<String>,
        model: impl Into<String>,
        structured_output_mode: LlmStructuredOutputMode,
        context_hash: String,
        schema_hash: String,
    ) -> Self {
        Self {
            schema_version: LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION.to_string(),
            report_level: level,
            request_id: request_id.into(),
            provider_id: provider_id.into(),
            model: model.into(),
            structured_output_mode,
            final_status: "generating".to_string(),
            repair_attempt_count: 0,
            repair_scope: None,
            candidate_hash: None,
            diagnostic_codes: Vec::new(),
            context_hash: (level == LlmPatchReportLevel::Trace).then_some(context_hash),
            schema_hash: (level == LlmPatchReportLevel::Trace).then_some(schema_hash),
            context_stale: false,
            cancelled: false,
            attempts: Vec::new(),
            lifecycle_state: LlmLifecycleState::Starting,
            terminal_status: None,
            cancel_requested: false,
            cancel_source: CancelSource::None,
            transport_cancel_capability: LlmTransportCancelCapability::AsyncAbort,
            transport_abort_requested: false,
            transport_abort_observed: false,
            task_join_status: LlmTaskJoinStatus::NotStarted,
            credential_owner_status: CredentialOwnerStatus::Held,
            local_execution_status: LlmLocalExecutionStatus::NotStarted,
            remote_execution_status: LlmRemoteExecutionStatus::NotStarted,
            cancel_latency_ms: None,
            shutdown_latency_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_patch_request_report_v3_serializes_bounded_repair_scope_evidence() {
        let mut report = LlmPatchRequestReport::started(
            LlmPatchReportLevel::Summary,
            "request-1",
            "provider",
            "model",
            LlmStructuredOutputMode::StrictJsonSchema,
            "context".to_string(),
            "schema".to_string(),
        );
        report.repair_scope = Some(LlmPatchRepairScopeEvidence {
            status: RepairScopeValidationStatus::ScopeUnprovableRestricted,
            initial_operation_count: None,
            repaired_operation_count: 1,
            changed_slots: vec![0],
            diagnostic_codes: vec!["project_patch_import.parse_failed".to_string()],
            rejection_code: None,
        });

        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value["schemaVersion"],
            LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION
        );
        assert_eq!(
            value["repairScope"]["status"],
            "scope_unprovable_restricted"
        );
        assert!(value.get("rawCandidate").is_none());
        assert!(value.get("prompt").is_none());
        assert_eq!(value["lifecycleState"], "starting");
        assert_eq!(value["credentialOwnerStatus"], "held");
        assert_eq!(value["remoteExecutionStatus"], "not_started");
    }

    #[test]
    fn llm_patch_request_report_summary_contains_no_request_owned_text() {
        let report = LlmPatchRequestReport::started(
            LlmPatchReportLevel::Summary,
            "request-secret-test",
            "provider",
            "model",
            LlmStructuredOutputMode::StrictJsonSchema,
            "context-secret-value".to_string(),
            "schema-secret-value".to_string(),
        );
        let encoded = serde_json::to_string(&report).unwrap();
        assert!(!encoded.contains("context-secret-value"));
        assert!(!encoded.contains("schema-secret-value"));
        assert!(!encoded.contains("Authorization"));
    }
}
