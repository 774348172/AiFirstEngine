mod applier;
mod context;
mod history;
mod import;
mod llm_http;
mod llm_repair;
mod llm_request;
mod llm_request_controller;
mod llm_source;
mod llm_transport;
mod model;
mod repair_scope;
mod schema;
mod session;
mod validator;

pub use applier::PatchApplier;
pub use context::{ProjectPatchLlmContextSnapshot, PROJECT_PATCH_LLM_CONTEXT_SCHEMA_VERSION};
pub use history::{PatchHistory, PatchHistoryEntry};
pub use import::ProjectPatchImportService;
pub use llm_repair::{
    build_project_patch_repair_prompt, diagnostic_fingerprint, import_diagnostics,
    project_patch_import_accepted, repair_decision, RepairDecision,
};
pub use llm_request::{
    LlmPatchAttemptSummary, LlmPatchRepairScopeEvidence, LlmPatchReportLevel,
    LlmPatchRequestReport, LLM_PATCH_REQUEST_REPORT_SCHEMA_VERSION,
};
pub use llm_request_controller::{
    validate_llm_join_timeout_fail_closed, CancelSource, CredentialOwnerStatus, LlmAsyncExecutor,
    LlmAttemptDecision, LlmCancelReceipt, LlmCredentialLease, LlmLifecycleDiagnostic,
    LlmLifecycleState, LlmLocalExecutionStatus, LlmRemoteExecutionStatus, LlmRepairSpec,
    LlmRequestController, LlmRequestEvent, LlmRequestId, LlmRequestSpec, LlmShutdownReceipt,
    LlmTaskJoinStatus, LlmTerminalStatus, LlmTransportCancelCapability, LLM_CANCEL_JOIN_DEADLINE,
    LLM_DROP_JOIN_BUDGET, LLM_SESSION_SHUTDOWN_DEADLINE,
};
pub use llm_source::{
    build_project_patch_generation_prompt, LlmPatchSourceConfig, LlmPatchSourceKind,
    LlmPatchSourceResult, LlmPatchSourceStatus, LlmStructuredOutputMode, LlmTransportConfig,
    RedactedSecret, ThinLlmPatchSource,
};
pub(crate) use llm_transport::{LlmTransport, ReqwestAsyncTransport};
pub use model::summarize_patch_history;
pub use model::{
    AssetPatchOperation, AuiPatchOperation, BuildPatchOperation, InputBindingProcessorPatch,
    InputPatchOperation, PatchApplyReport, PatchApplyStatus, PatchCapability, PatchDiagnostic,
    PatchDiagnosticSeverity, PatchHistorySummary, PatchOperation, PatchOperationApplyStatus,
    PatchOperationResult, PatchReviewModel, PatchRiskLevel, PatchSource, PatchValidationReport,
    PrefabPatchOperation, ProjectPatchDocument, ProjectPatchImportParseStatus,
    ProjectPatchImportProductizationReport, ProjectPatchImportProductizationStatus,
    ProjectPatchImportRequest, ProjectPatchImportResult, ProjectPatchImportSourceKind,
    ProjectPatchProductizationReport, ProjectPatchProductizationStatus, RulePatchOperation,
    ScenePatchOperation, PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION,
    PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION, PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION,
    PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION, PROJECT_PATCH_SCHEMA_VERSION,
};
pub use repair_scope::{
    validate_repair_scope, RepairScopePolicy, RepairScopeValidation, RepairScopeValidationStatus,
    REPAIR_SCOPE_UNPROVABLE_MAX_OPERATIONS,
};
pub use schema::{
    project_patch_json_schema, project_patch_json_schema_hash, project_patch_json_schema_string,
};
pub(crate) use session::ProjectFileSnapshotSet;
pub use validator::PatchValidator;
