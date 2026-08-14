use crate::ai_tool_catalog::{
    AiToolAvailabilityContext, AiToolCatalog, AiToolCatalogRequest,
    AiToolMutationAvailabilityState, AiToolReadAvailabilityState, AI_TOOL_CATALOG_SCHEMA_VERSION,
    AI_TOOL_CATALOG_V1_SCHEMA_VERSION,
};
use crate::{
    AssetImportConflictPolicy, CommandStatus, ControlledSourcePatchOperation, EditorSession,
    PatchOperation, PreparedProjectCandidatePayload, ProjectBuildExportEvidence,
    ProjectBuildExportInput, ProjectCandidateApplyReceipt, ProjectCandidateApproval,
    ProjectCandidateEntry, ProjectCandidateEnvelope, ProjectCandidatePayload,
    ProjectCandidatePrepareRequest, ProjectCandidateRollbackReceipt,
    ProjectCandidateValidationContext, ProjectCandidateValidationStatus, ProjectDeliveryTools,
    ProjectDeliveryVerifyEvidence, ProjectDeliveryVerifyInput, ProjectDiagnosticsInput,
    ProjectEvidenceReadInput, ProjectObjectReadInput, ProjectObservationIndex,
    ProjectObservationResult, ProjectPreviewCaptureKind, ProjectPreviewEvidence,
    ProjectPreviewFrameEvidence, ProjectPreviewFrameResultStatus, ProjectPreviewFrameTicket,
    ProjectReferencesInput, ProjectRuntimeCaptureIssueInput, ProjectRuntimeSourceKind,
    ProjectSearchInput, ProjectSourceSymbolsInput, ProjectUiExplainInput, ProjectUiLocateInput,
    ProjectUiLocateResult, ProjectUiOwnerTrace, ProjectUiOwnerTraceInput, ProjectVisualDiagnostics,
    ProjectVisualIssueBundle, PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION,
    PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION,
};
use editor_ui_model::{UiCommand, UiCommandPayload, UiCommandSource};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const AI_TOOL_DESCRIPTOR_SCHEMA_VERSION: &str = "ai-tool-descriptor.v1";
pub const AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION: &str = "ai-tool-inspect-request.v1";
pub const AI_TOOL_INSPECT_RESULT_SCHEMA_VERSION: &str = "ai-tool-inspect-result.v1";
pub const AI_TOOL_INVOCATION_SCHEMA_VERSION: &str = "ai-tool-invocation.v1";
pub const AI_CAPABILITY_GRANT_SCHEMA_VERSION: &str = "ai-capability-grant.v3";
pub const AI_TOOL_RESULT_SCHEMA_VERSION: &str = "ai-tool-result.v1";
pub const AI_TOOL_OPERATION_SCHEMA_VERSION: &str = "ai-tool-operation.v1";
pub const AI_TOOL_ACCEPTED_SCHEMA_VERSION: &str = "ai-tool-accepted.v1";
pub const AI_TOOL_MUTATION_RECEIPT_SCHEMA_VERSION: &str = "ai-tool-mutation-receipt.v1";
pub const AI_TOOL_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str = "ai-tool-rollback-receipt.v1";
pub const AI_TOOL_CANCELLATION_RECEIPT_SCHEMA_VERSION: &str = "ai-tool-cancellation-receipt.v1";
pub const AI_TOOL_KERNEL_JOURNAL_SCHEMA_VERSION: &str = "ai-tool-kernel-journal.v1";
pub const AI_TOOL_IMPLEMENTATION_VERSION_V1: &str = "1.0.0";
pub const EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION: &str = "external-project-rollback.v1";

pub const TOOL_ID_PROJECT_CREATE: &str = "project.create";
pub const TOOL_ID_PROJECT_INSPECT: &str = "project.inspect";
pub const TOOL_ID_PROJECT_MUTATE: &str = "project.mutate";
pub const TOOL_ID_PROJECT_ROLLBACK: &str = "project.rollback";
pub const TOOL_ID_PROJECT_PREVIEW: &str = "project.preview";
pub const TOOL_ID_PROJECT_SEARCH: &str = "project.search";
pub const TOOL_ID_PROJECT_READ_OBJECT: &str = "project.read_object";
pub const TOOL_ID_PROJECT_REFERENCES: &str = "project.references";
pub const TOOL_ID_PROJECT_SOURCE_SYMBOLS: &str = "project.source_symbols";
pub const TOOL_ID_PROJECT_DIAGNOSTICS: &str = "project.diagnostics";
pub const TOOL_ID_EVIDENCE_READ: &str = "evidence.read";
pub const TOOL_ID_RUNTIME_CAPTURE_ISSUE: &str = "runtime.capture_issue";
pub const TOOL_ID_UI_LOCATE: &str = "ui.locate";
pub const TOOL_ID_UI_EXPLAIN_VISIBILITY: &str = "ui.explain_visibility";
pub const TOOL_ID_PROJECT_TRACE_UI_OWNER: &str = "project.trace_ui_owner";
pub const TOOL_ID_PROJECT_BUILD_EXPORT: &str = "project.build_export";
pub const TOOL_ID_PROJECT_DELIVERY_VERIFY: &str = "project.delivery_verify";

const TOOL_KERNEL_JOURNAL_PATH: &str = "Library/AiCapability/tool-kernel-journal.json";
const MAX_PENDING_OPERATIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolSideEffect {
    None,
    ProjectRead,
    ProjectMutation,
    GeneratedFiles,
    ProcessSpawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolCapability {
    ReadProject,
    MutateProject,
    DeleteProjectContent,
    ChangeDependencies,
    AccessNetwork,
    SpendExternalBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolCostClass {
    None,
    LocalCompute,
    ExternalMetered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolDurationClass {
    Instant,
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolDescriptor {
    pub schema_version: String,
    pub tool_id: String,
    pub tool_version: String,
    pub summary: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub minimal_input_example: Value,
    pub side_effects: Vec<AiToolSideEffect>,
    pub required_capabilities: Vec<AiToolCapability>,
    pub changed_domains: Vec<String>,
    pub cost_class: AiToolCostClass,
    pub expected_duration_class: AiToolDurationClass,
    pub supports_dry_run: bool,
    pub supports_cancellation: bool,
    pub supports_rollback: bool,
    pub diagnostic_codes: Vec<String>,
    pub idempotency_class: String,
    pub preconditions: Vec<String>,
    pub progress_event_schema: Value,
    pub completion_evidence: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AiToolContractRegistry {
    descriptors: Vec<AiToolDescriptor>,
}

impl Default for AiToolContractRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AiToolContractRegistry {
    pub fn new() -> Self {
        Self {
            descriptors: builtin_descriptors(),
        }
    }

    pub fn descriptors(&self) -> &[AiToolDescriptor] {
        &self.descriptors
    }

    pub fn descriptor(&self, tool_id: &str) -> Option<&AiToolDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.tool_id == tool_id)
    }

    pub fn validate_direct_input(
        &self,
        tool_id: &str,
        direct_input: &Value,
    ) -> Result<(), AiToolKernelError> {
        let descriptor = self.descriptor(tool_id).ok_or_else(|| {
            contract_error(
                "ai_tool.contract_unknown_tool",
                format!("Tool '{tool_id}' is not present in the Tool Contract Registry."),
            )
        })?;
        validate_schema_value(&descriptor.input_schema, direct_input).map_err(|message| {
            contract_error(
                "ai_tool.direct_input_schema_invalid",
                format!("Direct input for '{tool_id}' is invalid: {message}"),
            )
        })
    }

    pub fn decode_inspect_request(
        &self,
        direct_input: Value,
    ) -> Result<AiToolInspectRequest, AiToolKernelError> {
        let canonical_input = self.canonical_direct_input(TOOL_ID_PROJECT_INSPECT, direct_input)?;
        decode_direct_value(TOOL_ID_PROJECT_INSPECT, canonical_input)
    }

    pub fn decode_invocation_payload(
        &self,
        tool_id: &str,
        direct_input: Value,
    ) -> Result<AiToolInvocationPayload, AiToolKernelError> {
        if tool_id == TOOL_ID_PROJECT_INSPECT {
            return Err(contract_error(
                "ai_tool.contract_inspect_not_invocation",
                "project.inspect uses the Tool Kernel inspect interface, not execute.",
            ));
        }
        let canonical_input = self.canonical_direct_input(tool_id, direct_input)?;
        match tool_id {
            TOOL_ID_PROJECT_CREATE => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectCreate),
            TOOL_ID_PROJECT_MUTATE => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectMutationIntent),
            TOOL_ID_PROJECT_ROLLBACK => {
                let input: ExternalProjectRollbackInput =
                    decode_direct_value(tool_id, canonical_input)?;
                validate_external_rollback_ref(&input.rollback_ref)?;
                Ok(AiToolInvocationPayload::ProjectRollbackRef(input))
            }
            TOOL_ID_PROJECT_PREVIEW => {
                let _: EmptyDirectInput = decode_direct_value(tool_id, canonical_input)?;
                Ok(AiToolInvocationPayload::Preview)
            }
            TOOL_ID_PROJECT_SEARCH => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectSearch),
            TOOL_ID_PROJECT_READ_OBJECT => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectReadObject),
            TOOL_ID_PROJECT_REFERENCES => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectReferences),
            TOOL_ID_PROJECT_SOURCE_SYMBOLS => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectSourceSymbols),
            TOOL_ID_PROJECT_DIAGNOSTICS => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectDiagnostics),
            TOOL_ID_EVIDENCE_READ => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::EvidenceRead),
            TOOL_ID_RUNTIME_CAPTURE_ISSUE => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::RuntimeCaptureIssue),
            TOOL_ID_UI_LOCATE => {
                decode_direct_value(tool_id, canonical_input).map(AiToolInvocationPayload::UiLocate)
            }
            TOOL_ID_UI_EXPLAIN_VISIBILITY => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::UiExplainVisibility),
            TOOL_ID_PROJECT_TRACE_UI_OWNER => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectTraceUiOwner),
            TOOL_ID_PROJECT_BUILD_EXPORT => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectBuildExport),
            TOOL_ID_PROJECT_DELIVERY_VERIFY => decode_direct_value(tool_id, canonical_input)
                .map(AiToolInvocationPayload::ProjectDeliveryVerify),
            _ => Err(contract_error(
                "ai_tool.contract_unknown_tool",
                format!("Tool '{tool_id}' is not present in the Tool Contract Registry."),
            )),
        }
    }

    fn canonical_direct_input(
        &self,
        tool_id: &str,
        mut direct_input: Value,
    ) -> Result<Value, AiToolKernelError> {
        self.validate_direct_input(tool_id, &direct_input)?;
        if let Some(schema_version) = internal_direct_input_schema_version(tool_id) {
            direct_input
                .as_object_mut()
                .expect("validated direct tool input must be an object")
                .insert("schemaVersion".to_string(), json!(schema_version));
        }
        Ok(direct_input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalProjectRollbackInput {
    pub schema_version: String,
    pub rollback_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyDirectInput {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolInspectKind {
    Project,
    GrantLineage { grant_digest: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolInspectRequest {
    pub schema_version: String,
    pub kind: AiToolInspectKind,
}

impl AiToolInspectRequest {
    pub fn project() -> Self {
        Self {
            schema_version: AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION.to_string(),
            kind: AiToolInspectKind::Project,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiProjectInspection {
    pub project_id: String,
    pub project_root: String,
    pub project_digest: String,
    pub runtime_source_kind: ProjectRuntimeSourceKind,
    pub runtime_module_id: String,
    pub runtime_interface_version: String,
    pub recorded_operation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resultKind", content = "result", rename_all = "snake_case")]
pub enum AiToolInspectPayload {
    Project(AiProjectInspection),
    GrantLineage(Option<AiGrantLineage>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolInspectResult {
    pub schema_version: String,
    pub payload: AiToolInspectPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCapabilityGrantKind {
    Read,
    ScopedMutation,
    Elevated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCapabilityScopeMode {
    ExactDomains,
    ProjectOwnedLowRisk,
    Elevated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMutationKind {
    ProjectPatch,
    ControlledSourcePatch,
    AssetImport,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiCapabilityGrant {
    pub schema_version: String,
    pub grant_id: String,
    pub kind: AiCapabilityGrantKind,
    pub scope_mode: AiCapabilityScopeMode,
    pub project_identity: String,
    pub user_visible_outcome_digest: String,
    pub initial_base_digest: String,
    pub scope_digests: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub allowed_mutation_kinds: Vec<AiMutationKind>,
    pub allow_delete: bool,
    pub allow_dependency_change: bool,
    pub allow_network: bool,
    pub external_cost_budget_microunits: u64,
    pub time_budget_ms: u64,
    pub max_mutation_count: u32,
    pub expires_at_epoch_ms: Option<u64>,
    pub issued_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_binding: Option<crate::AiGoalBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_envelope: Option<crate::AiRiskEnvelope>,
    pub grant_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiElevatedGrantSpec {
    pub grant_id: String,
    pub project_identity: String,
    pub user_visible_outcome_digest: String,
    pub base_digest: String,
    pub allowed_domains: Vec<String>,
    pub allowed_mutation_kinds: Vec<AiMutationKind>,
    pub allow_delete: bool,
    pub allow_dependency_change: bool,
    pub allow_network: bool,
    pub issued_by: String,
}

impl AiCapabilityGrant {
    pub fn read(
        grant_id: impl Into<String>,
        project_identity: impl Into<String>,
        base_digest: impl Into<String>,
        issued_by: impl Into<String>,
    ) -> Result<Self, AiToolKernelError> {
        let base_digest = base_digest.into();
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: grant_id.into(),
            kind: AiCapabilityGrantKind::Read,
            scope_mode: AiCapabilityScopeMode::ProjectOwnedLowRisk,
            project_identity: project_identity.into(),
            user_visible_outcome_digest: sha256_prefixed(b"read-project-facts"),
            initial_base_digest: base_digest.clone(),
            scope_digests: vec![base_digest],
            allowed_domains: vec!["project".to_string(), "preview".to_string()],
            allowed_mutation_kinds: Vec::new(),
            allow_delete: false,
            allow_dependency_change: false,
            allow_network: false,
            external_cost_budget_microunits: 0,
            time_budget_ms: 300_000,
            max_mutation_count: 0,
            expires_at_epoch_ms: None,
            issued_by: issued_by.into(),
            goal_binding: None,
            risk_envelope: None,
            grant_digest: String::new(),
        }
        .seal()
    }

    pub fn scoped_mutation(
        grant_id: impl Into<String>,
        project_identity: impl Into<String>,
        user_visible_outcome_digest: impl Into<String>,
        base_digest: impl Into<String>,
        allowed_domains: Vec<String>,
        allowed_mutation_kinds: Vec<AiMutationKind>,
        issued_by: impl Into<String>,
    ) -> Result<Self, AiToolKernelError> {
        let base_digest = base_digest.into();
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: grant_id.into(),
            kind: AiCapabilityGrantKind::ScopedMutation,
            scope_mode: AiCapabilityScopeMode::ExactDomains,
            project_identity: project_identity.into(),
            user_visible_outcome_digest: user_visible_outcome_digest.into(),
            initial_base_digest: base_digest.clone(),
            scope_digests: vec![base_digest],
            allowed_domains,
            allowed_mutation_kinds,
            allow_delete: false,
            allow_dependency_change: false,
            allow_network: false,
            external_cost_budget_microunits: 0,
            time_budget_ms: 900_000,
            max_mutation_count: 16,
            expires_at_epoch_ms: None,
            issued_by: issued_by.into(),
            goal_binding: None,
            risk_envelope: None,
            grant_digest: String::new(),
        }
        .seal()
    }

    pub fn project_owned_low_risk(
        grant_id: impl Into<String>,
        project_identity: impl Into<String>,
        user_visible_outcome_digest: impl Into<String>,
        base_digest: impl Into<String>,
        issued_by: impl Into<String>,
    ) -> Result<Self, AiToolKernelError> {
        let base_digest = base_digest.into();
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: grant_id.into(),
            kind: AiCapabilityGrantKind::ScopedMutation,
            scope_mode: AiCapabilityScopeMode::ProjectOwnedLowRisk,
            project_identity: project_identity.into(),
            user_visible_outcome_digest: user_visible_outcome_digest.into(),
            initial_base_digest: base_digest.clone(),
            scope_digests: vec![base_digest],
            allowed_domains: [
                "asset",
                "aui",
                "input",
                "prefab",
                "rule",
                "rollback",
                "runtime_module",
                "scene",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            allowed_mutation_kinds: vec![
                AiMutationKind::ProjectPatch,
                AiMutationKind::ControlledSourcePatch,
                AiMutationKind::AssetImport,
                AiMutationKind::Rollback,
            ],
            allow_delete: false,
            allow_dependency_change: false,
            allow_network: false,
            external_cost_budget_microunits: 0,
            time_budget_ms: 900_000,
            max_mutation_count: 16,
            expires_at_epoch_ms: None,
            issued_by: issued_by.into(),
            goal_binding: None,
            risk_envelope: None,
            grant_digest: String::new(),
        }
        .seal()
    }

    pub fn elevated(spec: AiElevatedGrantSpec) -> Result<Self, AiToolKernelError> {
        let base_digest = spec.base_digest;
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: spec.grant_id,
            kind: AiCapabilityGrantKind::Elevated,
            scope_mode: AiCapabilityScopeMode::Elevated,
            project_identity: spec.project_identity,
            user_visible_outcome_digest: spec.user_visible_outcome_digest,
            initial_base_digest: base_digest.clone(),
            scope_digests: vec![base_digest],
            allowed_domains: spec.allowed_domains,
            allowed_mutation_kinds: spec.allowed_mutation_kinds,
            allow_delete: spec.allow_delete,
            allow_dependency_change: spec.allow_dependency_change,
            allow_network: spec.allow_network,
            external_cost_budget_microunits: 0,
            time_budget_ms: 900_000,
            max_mutation_count: 16,
            expires_at_epoch_ms: None,
            issued_by: spec.issued_by,
            goal_binding: None,
            risk_envelope: None,
            grant_digest: String::new(),
        }
        .seal()
    }

    pub fn seal(mut self) -> Result<Self, AiToolKernelError> {
        normalize_sorted(&mut self.scope_digests);
        normalize_sorted(&mut self.allowed_domains);
        self.allowed_mutation_kinds.sort();
        self.allowed_mutation_kinds.dedup();
        self.grant_digest.clear();
        self.grant_digest = digest_serializable(&self, "capability grant")?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), AiToolKernelError> {
        self.validate_with_expiry(true)
    }

    fn validate_with_expiry(&self, enforce_expiry: bool) -> Result<(), AiToolKernelError> {
        if self.schema_version != AI_CAPABILITY_GRANT_SCHEMA_VERSION {
            return Err(kernel_error(
                "ai_tool.grant_schema_unsupported",
                "CapabilityGrant schema is unsupported.",
                "Regenerate the grant using ai-capability-grant.v3.",
            ));
        }
        if self.grant_id.trim().is_empty()
            || self.project_identity.trim().is_empty()
            || self.issued_by.trim().is_empty()
            || self.user_visible_outcome_digest.trim().is_empty()
        {
            return Err(kernel_error(
                "ai_tool.grant_identity_invalid",
                "CapabilityGrant identity, issuer, and outcome are required.",
                "Request a complete user-approved grant.",
            ));
        }
        let mut unsigned = self.clone();
        unsigned.grant_digest.clear();
        if digest_serializable(&unsigned, "capability grant")? != self.grant_digest {
            return Err(kernel_error(
                "ai_tool.grant_digest_mismatch",
                "CapabilityGrant content does not match its digest.",
                "Discard the modified grant and request a new one.",
            ));
        }
        if self.time_budget_ms == 0 {
            return Err(kernel_error(
                "ai_tool.grant_time_budget_exhausted",
                "CapabilityGrant has no execution time budget.",
                "Request a positive time budget.",
            ));
        }
        if (self.scope_mode == AiCapabilityScopeMode::Elevated)
            != (self.kind == AiCapabilityGrantKind::Elevated)
        {
            return Err(kernel_error(
                "ai_tool.grant_scope_mode_invalid",
                "Elevated scope mode and Elevated grant kind must be selected together.",
                "Issue a new internally consistent CapabilityGrant.",
            ));
        }
        if self.scope_mode == AiCapabilityScopeMode::ProjectOwnedLowRisk
            && (self.allow_delete || self.allow_dependency_change || self.allow_network)
        {
            return Err(kernel_error(
                "ai_tool.low_risk_grant_escalated",
                "ProjectOwnedLowRisk grant cannot authorize delete, dependency, or network effects.",
                "Use an explicit Elevated grant for the requested side effect.",
            ));
        }
        match (&self.goal_binding, &self.risk_envelope) {
            (Some(goal), Some(risk)) => {
                goal.validate_integrity().map_err(goal_grant_kernel_error)?;
                risk.validate_integrity().map_err(goal_grant_kernel_error)?;
                if goal.project_identity != self.project_identity
                    || goal.initial_project_digest != self.initial_base_digest
                    || risk.max_mutation_count != self.max_mutation_count
                    || risk.time_budget_ms != self.time_budget_ms
                    || risk.external_cost_budget_microunits != self.external_cost_budget_microunits
                    || risk.allow_delete != self.allow_delete
                    || risk.allow_dependency_change != self.allow_dependency_change
                    || risk.allow_network != self.allow_network
                {
                    return Err(kernel_error(
                        "ai_tool.goal_grant_mismatch",
                        "CapabilityGrant does not match its GoalBinding and RiskEnvelope.",
                        "Discard the grant and issue it from the approved goal grant spec.",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(kernel_error(
                    "ai_tool.goal_grant_incomplete",
                    "CapabilityGrant must include both GoalBinding and RiskEnvelope or neither.",
                    "Issue the grant from one complete approved goal grant spec.",
                ));
            }
        }
        if enforce_expiry
            && self
                .expires_at_epoch_ms
                .is_some_and(|expires| now_epoch_ms() >= expires)
        {
            return Err(kernel_error(
                "ai_tool.grant_expired",
                "CapabilityGrant has expired.",
                "Inspect the current project and request a fresh grant.",
            ));
        }
        Ok(())
    }

    pub fn validate_integrity(&self) -> Result<(), AiToolKernelError> {
        self.validate()
    }

    pub fn validate_rollback_integrity(&self) -> Result<(), AiToolKernelError> {
        self.validate_with_expiry(false)
    }

    pub fn project_owned_low_risk_for_goal(
        spec: crate::AiGoalGrantSpec,
    ) -> Result<Self, AiToolKernelError> {
        spec.validate_integrity().map_err(goal_grant_kernel_error)?;
        if spec.risk_envelope.risk_class != crate::AiGoalRiskClass::ProjectOwnedLowRisk {
            return Err(kernel_error(
                "ai_tool.goal_risk_class_invalid",
                "ProjectOwnedLowRisk grant requires a ProjectOwnedLowRisk RiskEnvelope.",
                "Use the grant constructor matching the approved risk class.",
            ));
        }
        let goal = spec.goal_binding;
        let risk = spec.risk_envelope;
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: format!("goal-grant-{}", goal.goal_id),
            kind: AiCapabilityGrantKind::ScopedMutation,
            scope_mode: AiCapabilityScopeMode::ProjectOwnedLowRisk,
            project_identity: goal.project_identity.clone(),
            user_visible_outcome_digest: goal.binding_digest.clone(),
            initial_base_digest: goal.initial_project_digest.clone(),
            scope_digests: vec![goal.initial_project_digest.clone()],
            allowed_domains: [
                "asset",
                "aui",
                "build",
                "input",
                "prefab",
                "rollback",
                "rule",
                "runtime_module",
                "scene",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            allowed_mutation_kinds: vec![
                AiMutationKind::ProjectPatch,
                AiMutationKind::ControlledSourcePatch,
                AiMutationKind::AssetImport,
                AiMutationKind::Rollback,
            ],
            allow_delete: risk.allow_delete,
            allow_dependency_change: risk.allow_dependency_change,
            allow_network: risk.allow_network,
            external_cost_budget_microunits: risk.external_cost_budget_microunits,
            time_budget_ms: risk.time_budget_ms,
            max_mutation_count: risk.max_mutation_count,
            expires_at_epoch_ms: spec.expires_at_epoch_ms,
            issued_by: spec.issued_by,
            goal_binding: Some(goal),
            risk_envelope: Some(risk),
            grant_digest: String::new(),
        }
        .seal()
    }

    pub fn elevated_for_goal(spec: crate::AiGoalGrantSpec) -> Result<Self, AiToolKernelError> {
        spec.validate_integrity().map_err(goal_grant_kernel_error)?;
        if spec.risk_envelope.risk_class != crate::AiGoalRiskClass::Elevated {
            return Err(kernel_error(
                "ai_tool.goal_risk_class_invalid",
                "Elevated goal grant requires an Elevated RiskEnvelope.",
                "Use the grant constructor matching the approved risk class.",
            ));
        }
        let goal = spec.goal_binding;
        let risk = spec.risk_envelope;
        Self {
            schema_version: AI_CAPABILITY_GRANT_SCHEMA_VERSION.to_string(),
            grant_id: format!("goal-grant-{}", goal.goal_id),
            kind: AiCapabilityGrantKind::Elevated,
            scope_mode: AiCapabilityScopeMode::Elevated,
            project_identity: goal.project_identity.clone(),
            user_visible_outcome_digest: goal.binding_digest.clone(),
            initial_base_digest: goal.initial_project_digest.clone(),
            scope_digests: vec![goal.initial_project_digest.clone()],
            allowed_domains: [
                "asset",
                "aui",
                "build",
                "input",
                "prefab",
                "rollback",
                "rule",
                "runtime_module",
                "scene",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            allowed_mutation_kinds: vec![
                AiMutationKind::ProjectPatch,
                AiMutationKind::ControlledSourcePatch,
                AiMutationKind::AssetImport,
                AiMutationKind::Rollback,
            ],
            allow_delete: risk.allow_delete,
            allow_dependency_change: risk.allow_dependency_change,
            allow_network: risk.allow_network,
            external_cost_budget_microunits: risk.external_cost_budget_microunits,
            time_budget_ms: risk.time_budget_ms,
            max_mutation_count: risk.max_mutation_count,
            expires_at_epoch_ms: spec.expires_at_epoch_ms,
            issued_by: spec.issued_by,
            goal_binding: Some(goal),
            risk_envelope: Some(risk),
            grant_digest: String::new(),
        }
        .seal()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiCandidateToolInput {
    pub envelope: ProjectCandidateEnvelope,
    pub source_file_path: Option<String>,
    pub controlled_source_patch_validation: Option<crate::ControlledSourcePatchValidationRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateDirectInput {
    pub requested_project_root: String,
    pub project_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "payloadKind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AiToolInvocationPayload {
    ProjectCreate(ProjectCreateDirectInput),
    ProjectMutationIntent(crate::ExternalProjectMutationIntent),
    BoundGoalMutation(crate::BoundGoalMutation),
    Candidate(AiCandidateToolInput),
    ProjectRollbackRef(ExternalProjectRollbackInput),
    RollbackCandidate {
        receipt: ProjectCandidateApplyReceipt,
    },
    Preview,
    ProjectSearch(ProjectSearchInput),
    ProjectReadObject(ProjectObjectReadInput),
    ProjectReferences(ProjectReferencesInput),
    ProjectSourceSymbols(ProjectSourceSymbolsInput),
    ProjectDiagnostics(ProjectDiagnosticsInput),
    EvidenceRead(ProjectEvidenceReadInput),
    RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput),
    UiLocate(ProjectUiLocateInput),
    UiExplainVisibility(ProjectUiExplainInput),
    ProjectTraceUiOwner(ProjectUiOwnerTraceInput),
    ProjectBuildExport(ProjectBuildExportInput),
    ProjectDeliveryVerify(ProjectDeliveryVerifyInput),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolInvocation {
    pub schema_version: String,
    pub invocation_id: String,
    pub tool_id: String,
    pub expected_project_digest: String,
    pub payload: AiToolInvocationPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolDiagnostic {
    pub severity: AiToolDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolExecutionStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolMutationReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub operation_id: String,
    pub tool_id: String,
    pub tool_version: String,
    pub grant_digest: String,
    pub project_identity: String,
    pub before_project_digest: String,
    pub after_project_digest: String,
    pub changed_paths_or_objects: Vec<String>,
    pub changed_domains: Vec<String>,
    pub candidate_digest: String,
    pub validation_digest: String,
    pub rollback_handle: String,
    pub candidate_receipt: ProjectCandidateApplyReceipt,
    pub duration_ms: u64,
    pub external_cost_microunits: u64,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolRollbackReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub operation_id: String,
    pub tool_id: String,
    pub grant_digest: String,
    pub project_identity: String,
    pub replaced_project_digest: String,
    pub restored_project_digest: String,
    pub candidate_rollback: ProjectCandidateRollbackReceipt,
    pub duration_ms: u64,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolPreviewEvidence {
    pub project_identity: String,
    pub project_digest: String,
    pub runtime_source_kind: ProjectRuntimeSourceKind,
    pub expected_module_id: String,
    pub linked_module_id: String,
    pub runtime_bind_receipt_digest: String,
    pub preview_package_report_path: Option<String>,
    pub present_report_path: Option<String>,
    pub frame_evidence_ref: String,
    pub frame_evidence_digest: String,
    pub screenshot_ref: String,
    pub screenshot_digest: String,
    pub frame_index: u64,
    pub frame_digest: String,
    pub capture_kind: ProjectPreviewCaptureKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outputKind", content = "output", rename_all = "snake_case")]
pub enum AiToolOutput {
    CandidateApplied(AiToolMutationReceipt),
    CandidateRolledBack(AiToolRollbackReceipt),
    Preview(AiToolPreviewEvidence),
    ProjectObservation(ProjectObservationResult),
    VisualIssueCaptured(ProjectVisualIssueBundle),
    VisualIssue(engine_runtime::visual_issue::VisualIssueBundle),
    UiLocated(ProjectUiLocateResult),
    UiOwnerTrace(ProjectUiOwnerTrace),
    ProjectBuildExport(ProjectBuildExportEvidence),
    ProjectDeliveryVerify(ProjectDeliveryVerifyEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateToolReceipt {
    pub status: String,
    pub receipt_id: String,
    pub requested_project_root: String,
    pub canonical_project_root: String,
    pub project_name: String,
    pub project_identity: String,
    pub project_digest: String,
    pub read_generation: u64,
    pub opened_in_editor: bool,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolResult {
    pub schema_version: String,
    pub status: AiToolExecutionStatus,
    pub tool_id: String,
    pub tool_version: String,
    pub operation_id: String,
    pub project_identity: Option<String>,
    pub facts: BTreeMap<String, String>,
    pub diagnostics: Vec<AiToolDiagnostic>,
    pub suggested_next_actions: Vec<String>,
    pub changed_domains: Vec<String>,
    pub output: Option<AiToolOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_ref: Option<String>,
    pub evidence_refs: Vec<String>,
    pub duration_ms: u64,
    pub external_cost_microunits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolOperationState {
    Queued,
    AwaitingUser,
    Preflight,
    Prepared,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolOperationTransition {
    pub state: AiToolOperationState,
    pub stage: String,
    pub at_epoch_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolOperationSnapshot {
    pub schema_version: String,
    pub operation_id: String,
    pub invocation_id: String,
    pub invocation_digest: String,
    pub tool_id: String,
    pub grant_digest: String,
    pub project_identity: String,
    pub state: AiToolOperationState,
    pub stage: String,
    pub started_at_epoch_ms: u64,
    pub completed_at_epoch_ms: Option<u64>,
    pub result: Option<AiToolResult>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub cancel_signal_sent: bool,
    #[serde(default)]
    pub commit_started: bool,
    #[serde(default)]
    pub transitions: Vec<AiToolOperationTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolAccepted {
    pub schema_version: String,
    pub operation_id: String,
    pub invocation_id: String,
    pub tool_id: String,
    pub project_identity: String,
    pub state: AiToolOperationState,
    pub accepted_at_epoch_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiToolStartOutcome {
    Accepted(AiToolAccepted),
    Terminal(AiToolResult),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiGrantLineage {
    pub grant_digest: String,
    pub current_project_digest: String,
    pub mutation_count: u32,
    pub consumed_time_ms: u64,
    pub consumed_external_cost_microunits: u64,
    pub receipt_digests: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiToolCancellationStatus {
    Cancelled,
    AlreadyTerminal,
    NotCancellable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolCancellationReceipt {
    pub schema_version: String,
    pub operation_id: String,
    pub grant_digest: String,
    pub status: AiToolCancellationStatus,
    pub cancelled_at_epoch_ms: u64,
    pub diagnostic_code: String,
    #[serde(default)]
    pub signal_sent: bool,
    #[serde(default)]
    pub child_termination_observed: bool,
    #[serde(default)]
    pub commit_started: bool,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiToolKernelError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl std::fmt::Display for AiToolKernelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AiToolKernelError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AiToolKernelJournal {
    schema_version: String,
    project_identity: String,
    lineages: BTreeMap<String, AiGrantLineage>,
    operations: BTreeMap<String, AiToolOperationSnapshot>,
    journal_digest: String,
}

impl AiToolKernelJournal {
    fn empty(project_identity: &str) -> Self {
        Self {
            schema_version: AI_TOOL_KERNEL_JOURNAL_SCHEMA_VERSION.to_string(),
            project_identity: project_identity.to_string(),
            lineages: BTreeMap::new(),
            operations: BTreeMap::new(),
            journal_digest: String::new(),
        }
    }

    fn seal(&mut self) -> Result<(), AiToolKernelError> {
        self.journal_digest.clear();
        self.journal_digest = digest_serializable(self, "tool kernel journal")?;
        Ok(())
    }

    fn validate(&self, project_identity: &str) -> Result<(), AiToolKernelError> {
        if self.schema_version != AI_TOOL_KERNEL_JOURNAL_SCHEMA_VERSION
            || self.project_identity != project_identity
        {
            return Err(kernel_error(
                "ai_tool.journal_binding_invalid",
                "Tool Kernel journal is not bound to the active project or schema.",
                "Restore the matching journal or quarantine the invalid Library metadata.",
            ));
        }
        let mut unsigned = self.clone();
        unsigned.journal_digest.clear();
        if digest_serializable(&unsigned, "tool kernel journal")? != self.journal_digest {
            return Err(kernel_error(
                "ai_tool.journal_digest_mismatch",
                "Tool Kernel journal content does not match its digest.",
                "Do not infer operation or authorization state from the modified journal.",
            ));
        }
        Ok(())
    }
}

struct PendingAiToolOperation {
    invocation: AiToolInvocation,
    grant: AiCapabilityGrant,
    cancellation: runtime_cli::BoundedChildProcessCancellation,
    prepared_preview: Option<PreparedProjectOwnedPreview>,
}

struct PreparedProjectOwnedPreview {
    project_identity: String,
    project_digest: String,
    runtime_source_kind: ProjectRuntimeSourceKind,
    expected_module_id: String,
    linked_module_id: String,
    runtime_bind_receipt_digest: String,
    preview_package_report_path: Option<String>,
    present_report_path: Option<String>,
    ticket: ProjectPreviewFrameTicket,
}

#[derive(Default)]
pub struct AiCapabilityToolKernel {
    loaded_project_identity: Option<String>,
    lineages: BTreeMap<String, AiGrantLineage>,
    operations: BTreeMap<String, AiToolOperationSnapshot>,
    pending: BTreeMap<String, PendingAiToolOperation>,
    pending_order: VecDeque<String>,
    launcher_project_create_replays: BTreeMap<String, (String, AiToolResult)>,
}

impl AiCapabilityToolKernel {
    pub fn operation_id_for_invocation(
        invocation: &AiToolInvocation,
        grant_digest: &str,
    ) -> String {
        let authority_identity = match &invocation.payload {
            AiToolInvocationPayload::BoundGoalMutation(bound) => {
                format!("{}|{}", bound.client_session_id, bound.goal_digest)
            }
            _ => grant_digest.to_string(),
        };
        operation_id_for(&invocation.invocation_id, &authority_identity)
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant_lineage(&self, grant_digest: &str) -> Option<AiGrantLineage> {
        self.lineages.get(grant_digest).cloned()
    }

    pub fn execute_launcher_project_create(
        &mut self,
        session: &mut EditorSession,
        invocation: AiToolInvocation,
        read_generation: u64,
    ) -> AiToolResult {
        let started = Instant::now();
        let invocation_digest = digest_serializable(&invocation, "project.create invocation")
            .unwrap_or_else(|_| sha256_prefixed(invocation.invocation_id.as_bytes()));
        let operation_id =
            operation_id_for(&invocation.invocation_id, "editor-instance-project-create");
        if let Some((existing_digest, existing_result)) = self
            .launcher_project_create_replays
            .get(&invocation.invocation_id)
        {
            if existing_digest != &invocation_digest {
                return failed_result(
                    TOOL_ID_PROJECT_CREATE,
                    &operation_id,
                    existing_result.project_identity.clone(),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.invocation_replay_mismatch",
                        "Invocation id was reused with different project.create input.",
                        "Use a new invocation id for the changed request.",
                    ),
                );
            }
            let mut replay = existing_result.clone();
            replay
                .facts
                .insert("replayed".to_string(), "true".to_string());
            return replay;
        }
        if invocation.schema_version != AI_TOOL_INVOCATION_SCHEMA_VERSION
            || invocation.tool_id != TOOL_ID_PROJECT_CREATE
        {
            return failed_result(
                TOOL_ID_PROJECT_CREATE,
                &operation_id,
                None,
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.project_create_invocation_invalid",
                    "project.create received an incompatible invocation envelope.",
                    "Use the registered typed project.create contract.",
                ),
            );
        }
        let AiToolInvocationPayload::ProjectCreate(input) = &invocation.payload else {
            return failed_result(
                TOOL_ID_PROJECT_CREATE,
                &operation_id,
                None,
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.project_create_payload_invalid",
                    "project.create requires its exact typed payload.",
                    "Use the registered typed project.create contract.",
                ),
            );
        };
        let input = input.clone();
        let created = match session.create_project_for_ai_tool(
            Path::new(&input.requested_project_root),
            &input.project_name,
        ) {
            Ok(created) => created,
            Err(error) => {
                return failed_result(
                    TOOL_ID_PROJECT_CREATE,
                    &operation_id,
                    None,
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        &error.code,
                        error.message,
                        "Correct the project root or name and use a new invocation.",
                    ),
                );
            }
        };
        let binding = match ProjectCandidateEntry::inspect_project_binding(session) {
            Ok(binding) => binding,
            Err(error) => {
                return failed_result(
                    TOOL_ID_PROJECT_CREATE,
                    &operation_id,
                    Some(created.project_identity),
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                );
            }
        };
        let receipt_id = format!(
            "project-create-receipt-{}",
            sha256_prefixed(
                format!(
                    "{}|{}|{}",
                    invocation.invocation_id,
                    created.canonical_project_root,
                    binding.project_digest
                )
                .as_bytes()
            )
            .trim_start_matches("sha256:")
            .chars()
            .take(24)
            .collect::<String>()
        );
        let receipt = ProjectCreateToolReceipt {
            status: "created".to_string(),
            receipt_id,
            requested_project_root: created.requested_project_root,
            canonical_project_root: created.canonical_project_root,
            project_name: created.project_name,
            project_identity: binding.project_id.clone(),
            project_digest: binding.project_digest.clone(),
            read_generation,
            opened_in_editor: true,
            replayed: false,
        };
        let result = project_create_completed_result(
            &operation_id,
            receipt,
            started.elapsed().as_millis() as u64,
        );
        self.launcher_project_create_replays.insert(
            invocation.invocation_id,
            (invocation_digest, result.clone()),
        );
        result
    }

    pub fn catalog(
        &self,
        request: AiToolCatalogRequest,
    ) -> Result<AiToolCatalog, AiToolKernelError> {
        let tools = AiToolContractRegistry::new().descriptors;
        match request.schema_version.as_str() {
            AI_TOOL_CATALOG_V1_SCHEMA_VERSION => Ok(AiToolCatalog::v1(tools)),
            AI_TOOL_CATALOG_SCHEMA_VERSION => Ok(AiToolCatalog::v2(
                tools,
                AiToolAvailabilityContext::default(),
            )),
            _ => Err(kernel_error(
                "ai_tool.catalog_schema_unsupported",
                "Tool catalog request schema is unsupported.",
                "Request ai-tool-catalog.v2 or ai-tool-catalog.v1.",
            )),
        }
    }

    pub fn catalog_for_session(
        &self,
        session: &EditorSession,
        request: AiToolCatalogRequest,
    ) -> Result<AiToolCatalog, AiToolKernelError> {
        if request.schema_version == AI_TOOL_CATALOG_V1_SCHEMA_VERSION {
            return self.catalog(request);
        }
        if request.schema_version != AI_TOOL_CATALOG_SCHEMA_VERSION {
            return self.catalog(request);
        }
        Ok(AiToolCatalog::v2(
            AiToolContractRegistry::new().descriptors,
            self.availability_context(session),
        ))
    }

    pub fn catalog_for_session_with_context(
        &self,
        request: AiToolCatalogRequest,
        context: AiToolAvailabilityContext,
    ) -> Result<AiToolCatalog, AiToolKernelError> {
        match request.schema_version.as_str() {
            AI_TOOL_CATALOG_V1_SCHEMA_VERSION => self.catalog(request),
            AI_TOOL_CATALOG_SCHEMA_VERSION => Ok(AiToolCatalog::v2(
                AiToolContractRegistry::new().descriptors,
                context,
            )),
            _ => self.catalog(request),
        }
    }

    pub fn availability_context(&self, session: &EditorSession) -> AiToolAvailabilityContext {
        let Some(project) = session.active_project_session() else {
            return AiToolAvailabilityContext::default();
        };
        let runtime_descriptor = session
            .linked_project_runtimes
            .descriptor_for_module_id(&project.manifest.runtime_module.module_id);
        let runtime_ready = runtime_descriptor.is_some_and(|descriptor| {
            descriptor.module_id == project.manifest.runtime_module.module_id
                && descriptor.interface_version == project.manifest.runtime_module.interface_version
        });
        let runtime_binding_digest = runtime_descriptor.map(|descriptor| {
            digest_serializable(
                &(&project.manifest.runtime_module, descriptor),
                "runtime binding",
            )
            .expect("runtime binding is serializable")
        });
        AiToolAvailabilityContext {
            basis: crate::AiToolAvailabilityBasis {
                project_identity: Some(project.manifest.project_id.clone()),
                project_digest: None,
                read_generation: None,
                runtime_binding_digest,
                access_generation: None,
                operation_generation: Some(self.operations.len() as u64),
            },
            read_state: AiToolReadAvailabilityState::Active,
            mutation_state: AiToolMutationAvailabilityState::NotRequested,
            runtime_ready,
            delivery_supported: cfg!(target_os = "windows"),
            operation_conflict: self.pending.len() >= MAX_PENDING_OPERATIONS,
            rollback_lineage_known: self.operations.values().any(|operation| {
                operation.result.as_ref().is_some_and(|result| {
                    matches!(result.output, Some(AiToolOutput::CandidateApplied(_)))
                })
            }),
        }
    }

    pub fn inspect(
        &mut self,
        session: &EditorSession,
        request: AiToolInspectRequest,
    ) -> Result<AiToolInspectResult, AiToolKernelError> {
        if request.schema_version != AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION {
            return Err(kernel_error(
                "ai_tool.inspect_schema_unsupported",
                "Inspect request schema is unsupported.",
                "Regenerate the request using ai-tool-inspect-request.v1.",
            ));
        }
        let binding = ProjectCandidateEntry::inspect_project_binding(session)
            .map_err(candidate_entry_error)?;
        self.hydrate(session, &binding.project_id)?;
        let payload = match request.kind {
            AiToolInspectKind::Project => {
                let project = session.active_project_session().ok_or_else(|| {
                    kernel_error(
                        "ai_tool.project_required",
                        "Project inspection requires an active project.",
                        "Open or create the target project.",
                    )
                })?;
                AiToolInspectPayload::Project(AiProjectInspection {
                    project_id: binding.project_id,
                    project_root: binding.project_root,
                    project_digest: binding.project_digest,
                    runtime_source_kind: project.manifest.runtime_module.resolved_source_kind(),
                    runtime_module_id: project.manifest.runtime_module.module_id.clone(),
                    runtime_interface_version: project
                        .manifest
                        .runtime_module
                        .interface_version
                        .clone(),
                    recorded_operation_count: self.operations.len(),
                })
            }
            AiToolInspectKind::GrantLineage { grant_digest } => {
                AiToolInspectPayload::GrantLineage(self.lineages.get(&grant_digest).cloned())
            }
        };
        Ok(AiToolInspectResult {
            schema_version: AI_TOOL_INSPECT_RESULT_SCHEMA_VERSION.to_string(),
            payload,
        })
    }

    pub fn execute(
        &mut self,
        session: &mut EditorSession,
        invocation: AiToolInvocation,
        grant: &AiCapabilityGrant,
    ) -> AiToolResult {
        let requires_frame_evidence =
            matches!(&invocation.payload, AiToolInvocationPayload::Preview);
        match self.start(session, invocation, grant) {
            AiToolStartOutcome::Terminal(result) => result,
            AiToolStartOutcome::Accepted(accepted) => {
                if requires_frame_evidence {
                    self.pending.remove(&accepted.operation_id);
                    self.pending_order
                        .retain(|queued| queued != &accepted.operation_id);
                    session.discard_project_preview_frame_capture(&accepted.operation_id);
                    let result = failed_result(
                        TOOL_ID_PROJECT_PREVIEW,
                        &accepted.operation_id,
                        Some(accepted.project_identity),
                        0,
                        kernel_error(
                            "ai_tool.preview_async_execution_required",
                            "Preview requires the real-window presented-frame evidence barrier.",
                            "Use start/observe/pump and keep the native Editor window running until exact frame evidence is captured.",
                        ),
                    );
                    self.finish_operation(
                        session,
                        &accepted.operation_id,
                        result,
                        AiToolOperationState::Failed,
                    );
                    return self
                        .observe(&accepted.operation_id)
                        .expect("direct Preview failure must remain observable")
                        .result
                        .expect("direct Preview failure must remain observable");
                }
                loop {
                    self.pump_operations(session, 1);
                    let operation = self
                        .observe(&accepted.operation_id)
                        .expect("accepted operation must remain observable");
                    if let Some(result) = operation.result {
                        break result;
                    }
                }
            }
        }
    }

    pub fn start(
        &mut self,
        session: &EditorSession,
        invocation: AiToolInvocation,
        grant: &AiCapabilityGrant,
    ) -> AiToolStartOutcome {
        let started = Instant::now();
        let started_at = now_epoch_ms();
        let invocation_digest = digest_serializable(&invocation, "tool invocation")
            .unwrap_or_else(|_| sha256_prefixed(invocation.invocation_id.as_bytes()));
        let operation_id = Self::operation_id_for_invocation(&invocation, &grant.grant_digest);

        let binding = match ProjectCandidateEntry::inspect_project_binding(session) {
            Ok(binding) => binding,
            Err(error) => {
                return AiToolStartOutcome::Terminal(failed_result(
                    &invocation.tool_id,
                    &operation_id,
                    None,
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                ));
            }
        };
        if let Err(error) = self.hydrate(session, &binding.project_id) {
            return AiToolStartOutcome::Terminal(failed_result(
                &invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                error,
            ));
        }

        if let Some(existing) = self.operations.get(&operation_id) {
            if existing.invocation_digest != invocation_digest {
                return AiToolStartOutcome::Terminal(failed_result(
                    &invocation.tool_id,
                    &operation_id,
                    Some(binding.project_id),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.invocation_replay_mismatch",
                        "Invocation id was reused with different content.",
                        "Use a new invocation id for the changed request.",
                    ),
                ));
            }
            if let Some(result) = &existing.result {
                return AiToolStartOutcome::Terminal(result.clone());
            }
            return AiToolStartOutcome::Accepted(accepted_from(existing));
        }

        if let Err(error) = validate_invocation_preflight(&invocation, grant, &binding) {
            return AiToolStartOutcome::Terminal(failed_result(
                &invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                error,
            ));
        }
        if let Err(error) = self.validate_tool_authorization(&invocation, grant, &binding) {
            return AiToolStartOutcome::Terminal(failed_result(
                &invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                error,
            ));
        }
        if let Err(error) = validate_candidate_prepare_preflight(session, &invocation) {
            return AiToolStartOutcome::Terminal(failed_result(
                &invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                error,
            ));
        }
        if self.pending.len() >= MAX_PENDING_OPERATIONS {
            return AiToolStartOutcome::Terminal(failed_result(
                &invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.operation_backpressure",
                    "The bounded Tool Kernel operation queue is full.",
                    "Observe or cancel existing operations before submitting more work.",
                ),
            ));
        }

        let queued = AiToolOperationSnapshot {
            schema_version: AI_TOOL_OPERATION_SCHEMA_VERSION.to_string(),
            operation_id: operation_id.clone(),
            invocation_id: invocation.invocation_id.clone(),
            invocation_digest,
            tool_id: invocation.tool_id.clone(),
            grant_digest: grant.grant_digest.clone(),
            project_identity: binding.project_id.clone(),
            state: AiToolOperationState::Queued,
            stage: "queued".to_string(),
            started_at_epoch_ms: started_at,
            completed_at_epoch_ms: None,
            result: None,
            artifact_refs: Vec::new(),
            cancel_signal_sent: false,
            commit_started: false,
            transitions: vec![AiToolOperationTransition {
                state: AiToolOperationState::Queued,
                stage: "queued".to_string(),
                at_epoch_ms: started_at,
            }],
        };
        self.operations.insert(operation_id.clone(), queued);
        self.pending.insert(
            operation_id.clone(),
            PendingAiToolOperation {
                invocation,
                grant: grant.clone(),
                cancellation: runtime_cli::BoundedChildProcessCancellation::default(),
                prepared_preview: None,
            },
        );
        self.pending_order.push_back(operation_id.clone());
        if let Err(error) = self.persist(session) {
            self.operations.remove(&operation_id);
            let pending = self
                .pending
                .remove(&operation_id)
                .expect("pending operation");
            self.pending_order.retain(|queued| queued != &operation_id);
            return AiToolStartOutcome::Terminal(failed_result(
                &pending.invocation.tool_id,
                &operation_id,
                Some(binding.project_id),
                started.elapsed().as_millis() as u64,
                error,
            ));
        }
        AiToolStartOutcome::Accepted(accepted_from(
            self.operations
                .get(&operation_id)
                .expect("queued operation must be recorded"),
        ))
    }

    pub fn pump_operations(&mut self, session: &mut EditorSession, max_steps: usize) -> usize {
        let mut processed = 0;
        while processed < max_steps {
            let Some(operation_id) = self.pending_order.pop_front() else {
                break;
            };
            let state = self
                .operations
                .get(&operation_id)
                .map(|operation| operation.state);
            match state {
                Some(AiToolOperationState::Queued) => {
                    transition_operation(
                        self.operations.get_mut(&operation_id).expect("operation"),
                        AiToolOperationState::Preflight,
                        "preflight",
                    );
                    self.pending_order.push_back(operation_id);
                    let _ = self.persist(session);
                }
                Some(AiToolOperationState::Preflight) => {
                    transition_operation(
                        self.operations.get_mut(&operation_id).expect("operation"),
                        AiToolOperationState::Prepared,
                        "prepared",
                    );
                    self.pending_order.push_back(operation_id);
                    let _ = self.persist(session);
                }
                Some(AiToolOperationState::Prepared) => {
                    transition_operation(
                        self.operations.get_mut(&operation_id).expect("operation"),
                        AiToolOperationState::Running,
                        "running",
                    );
                    let _ = self.persist(session);
                    self.finish_pending_operation(session, &operation_id);
                }
                Some(AiToolOperationState::Running) => {
                    self.finish_pending_operation(session, &operation_id);
                }
                Some(AiToolOperationState::Cancelling) => {
                    self.finish_cancelled_operation(session, &operation_id);
                }
                _ => {
                    self.pending.remove(&operation_id);
                }
            }
            processed += 1;
        }
        processed
    }

    fn finish_pending_operation(&mut self, session: &mut EditorSession, operation_id: &str) {
        let Some(pending) = self.pending.remove(operation_id) else {
            self.finish_interrupted_operation(session, operation_id);
            return;
        };
        let binding = match ProjectCandidateEntry::inspect_project_binding(session) {
            Ok(binding) => binding,
            Err(error) => {
                let result = failed_result(
                    &pending.invocation.tool_id,
                    operation_id,
                    None,
                    0,
                    candidate_entry_error(error),
                );
                self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
                return;
            }
        };
        if matches!(pending.invocation.payload, AiToolInvocationPayload::Preview) {
            self.advance_pending_preview(session, operation_id, &binding, pending);
            return;
        }
        let result = self.execute_inner(
            session,
            &pending.invocation,
            &pending.grant,
            &binding,
            operation_id,
            Instant::now(),
            &pending.cancellation,
        );
        let state = if pending.cancellation.is_cancelled() {
            AiToolOperationState::Cancelled
        } else {
            match result.status {
                AiToolExecutionStatus::Completed => AiToolOperationState::Completed,
                AiToolExecutionStatus::Failed => AiToolOperationState::Failed,
            }
        };
        self.finish_operation(session, operation_id, result, state);
    }

    fn advance_pending_preview(
        &mut self,
        session: &mut EditorSession,
        operation_id: &str,
        binding: &crate::ProjectCandidateProjectBinding,
        mut pending: PendingAiToolOperation,
    ) {
        let duration_ms = self
            .operations
            .get(operation_id)
            .map(|operation| now_epoch_ms().saturating_sub(operation.started_at_epoch_ms))
            .unwrap_or_default();
        if pending.cancellation.is_cancelled() {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    "ai_tool.operation_cancelled_before_frame_evidence",
                    "Preview was cancelled before exact presented-frame evidence was retained.",
                    "Observe the terminal cancellation receipt before retrying.",
                ),
            );
            self.finish_operation(
                session,
                operation_id,
                result,
                AiToolOperationState::Cancelled,
            );
            return;
        }

        if pending.prepared_preview.is_none() {
            match prepare_project_owned_preview(session, operation_id) {
                Ok(prepared) => {
                    pending.prepared_preview = Some(prepared);
                    if let Some(operation) = self.operations.get_mut(operation_id) {
                        transition_operation(
                            operation,
                            AiToolOperationState::Running,
                            "awaiting_frame_evidence",
                        );
                    }
                    self.pending.insert(operation_id.to_string(), pending);
                    self.pending_order.push_back(operation_id.to_string());
                    let _ = self.persist(session);
                }
                Err(error) => {
                    let result = failed_result(
                        TOOL_ID_PROJECT_PREVIEW,
                        operation_id,
                        Some(binding.project_id.clone()),
                        duration_ms,
                        error,
                    );
                    self.finish_operation(
                        session,
                        operation_id,
                        result,
                        AiToolOperationState::Failed,
                    );
                }
            }
            return;
        }

        let prepared = pending
            .prepared_preview
            .as_ref()
            .expect("prepared Preview must be present");
        if binding.project_id != prepared.project_identity
            || binding.project_digest != prepared.project_digest
        {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    "ai_tool.preview_project_drifted_while_awaiting_frame",
                    "Project identity or digest changed while Preview awaited frame evidence.",
                    "Reconnect to the current project and start a fresh Preview operation.",
                ),
            );
            self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
            return;
        }

        let Some(frame_result) = session.project_preview_frame_result().cloned() else {
            self.pending.insert(operation_id.to_string(), pending);
            self.pending_order.push_back(operation_id.to_string());
            return;
        };
        if frame_result.operation_id != operation_id {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    "ai_tool.preview_frame_result_operation_mismatch",
                    "Presented-frame result belongs to a different Preview operation.",
                    "Discard the cross-operation result and start a fresh Preview.",
                ),
            );
            self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
            return;
        }
        if frame_result.status == ProjectPreviewFrameResultStatus::Failed {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    frame_result
                        .diagnostic_code
                        .as_deref()
                        .unwrap_or("ai_tool.preview_frame_evidence_failed"),
                    frame_result
                        .diagnostic_message
                        .unwrap_or_else(|| "Presented-frame evidence capture failed.".to_string()),
                    "Inspect the real-window readback diagnostic and retry Preview after repair.",
                ),
            );
            self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
            return;
        }
        let Some(evidence_ref) = frame_result.evidence_ref else {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    "ai_tool.preview_frame_evidence_ref_missing",
                    "Captured Preview frame result did not include an evidence reference.",
                    "Treat the capture as invalid and retry after repairing the receipt path.",
                ),
            );
            self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
            return;
        };
        let project = match session.active_project_session() {
            Some(project) => project,
            None => {
                session.discard_project_preview_frame_capture(operation_id);
                let result = failed_result(
                    TOOL_ID_PROJECT_PREVIEW,
                    operation_id,
                    None,
                    duration_ms,
                    kernel_error(
                        "ai_tool.project_required",
                        "Project closed before Preview frame evidence could be verified.",
                        "Open the project and start a fresh Preview.",
                    ),
                );
                self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
                return;
            }
        };
        let frame = match ProjectPreviewEvidence::validate_frame(
            project.write_scope(),
            &prepared.ticket,
            &evidence_ref,
        ) {
            Ok(frame) => frame,
            Err(error) => {
                session.discard_project_preview_frame_capture(operation_id);
                let result = failed_result(
                    TOOL_ID_PROJECT_PREVIEW,
                    operation_id,
                    Some(binding.project_id.clone()),
                    duration_ms,
                    kernel_error(
                        error.code,
                        error.message,
                        "Discard the invalid frame evidence and capture the exact shared texture again.",
                    ),
                );
                self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
                return;
            }
        };
        if frame_result.captured_evidence.as_ref() != Some(&frame) {
            session.discard_project_preview_frame_capture(operation_id);
            let result = failed_result(
                TOOL_ID_PROJECT_PREVIEW,
                operation_id,
                Some(binding.project_id.clone()),
                duration_ms,
                kernel_error(
                    "ai_tool.preview_frame_receipt_mismatch",
                    "Persisted Preview frame evidence no longer matches the trusted capture receipt.",
                    "Discard the modified evidence and capture the exact shared texture again.",
                ),
            );
            self.finish_operation(session, operation_id, result, AiToolOperationState::Failed);
            return;
        }
        let mut result = completed_preview_result(prepared, frame, evidence_ref, duration_ms);
        normalize_sorted(&mut result.evidence_refs);
        session.complete_project_preview_frame_capture(operation_id);
        self.finish_operation(
            session,
            operation_id,
            result,
            AiToolOperationState::Completed,
        );
    }

    fn finish_operation(
        &mut self,
        session: &EditorSession,
        operation_id: &str,
        mut result: AiToolResult,
        state: AiToolOperationState,
    ) {
        if let Some(operation) = self.operations.get_mut(operation_id) {
            operation.artifact_refs = result.evidence_refs.clone();
            transition_operation(operation, state, "terminal");
            operation.completed_at_epoch_ms = Some(now_epoch_ms());
            operation.result = Some(result.clone());
        }
        if let Err(error) = self.persist(session) {
            result.status = AiToolExecutionStatus::Failed;
            result.diagnostics.push(diagnostic_from_error(error));
            if let Some(operation) = self.operations.get_mut(operation_id) {
                transition_operation(operation, AiToolOperationState::Failed, "terminal");
                operation.result = Some(result);
            }
        }
    }

    fn finish_cancelled_operation(&mut self, session: &mut EditorSession, operation_id: &str) {
        let tool_id = self
            .operations
            .get(operation_id)
            .map(|operation| operation.tool_id.clone())
            .unwrap_or_default();
        let project_identity = self
            .operations
            .get(operation_id)
            .map(|operation| operation.project_identity.clone());
        self.pending.remove(operation_id);
        session.discard_project_preview_frame_capture(operation_id);
        let result = failed_result(
            &tool_id,
            operation_id,
            project_identity,
            0,
            kernel_error(
                "ai_tool.operation_cancelled",
                "Operation was cancelled before commit.",
                "Observe the terminal operation before submitting a replacement.",
            ),
        );
        self.finish_operation(
            session,
            operation_id,
            result,
            AiToolOperationState::Cancelled,
        );
    }

    fn finish_interrupted_operation(&mut self, session: &EditorSession, operation_id: &str) {
        let Some(operation) = self.operations.get(operation_id) else {
            return;
        };
        let result = failed_result(
            &operation.tool_id,
            operation_id,
            Some(operation.project_identity.clone()),
            0,
            kernel_error(
                "ai_tool.operation_interrupted",
                "The Editor restarted before the durable operation reached a terminal state.",
                "Reinspect project state and submit a new invocation if the outcome is still needed.",
            ),
        );
        self.finish_operation(
            session,
            operation_id,
            result,
            AiToolOperationState::Interrupted,
        );
    }

    pub fn observe(
        &self,
        operation_id: &str,
    ) -> Result<AiToolOperationSnapshot, AiToolKernelError> {
        self.operations.get(operation_id).cloned().ok_or_else(|| {
            kernel_error(
                "ai_tool.operation_unknown",
                "Operation id is not present in the hydrated project journal.",
                "Inspect the active project before observing a persisted operation.",
            )
        })
    }

    pub fn invalidate_operation_authority(
        &mut self,
        operation_id: &str,
    ) -> Option<AiToolOperationSnapshot> {
        let operation = self.operations.get(operation_id)?.clone();
        if matches!(
            operation.state,
            AiToolOperationState::Completed
                | AiToolOperationState::Failed
                | AiToolOperationState::Cancelled
                | AiToolOperationState::Interrupted
        ) {
            return Some(operation);
        }
        self.pending.remove(operation_id);
        self.pending_order
            .retain(|pending_id| pending_id != operation_id);
        let result = failed_result(
            &operation.tool_id,
            operation_id,
            Some(operation.project_identity),
            0,
            kernel_error(
                "ai_tool.operation_authority_invalidated",
                "The Gateway session disconnected before the operation reached a terminal state.",
                "Inspect project state before submitting a replacement mutation.",
            ),
        );
        if let Some(operation) = self.operations.get_mut(operation_id) {
            transition_operation(operation, AiToolOperationState::Interrupted, "terminal");
            operation.completed_at_epoch_ms = Some(now_epoch_ms());
            operation.result = Some(result);
            return Some(operation.clone());
        }
        None
    }

    pub fn invalidate_project_context_operations(
        &mut self,
        project_identity: &str,
    ) -> BTreeMap<String, AiToolOperationSnapshot> {
        let operation_ids = self
            .operations
            .iter()
            .filter(|(_, operation)| operation.project_identity == project_identity)
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();

        for operation_id in &operation_ids {
            let Some(operation) = self.operations.get(operation_id).cloned() else {
                continue;
            };
            if matches!(
                operation.state,
                AiToolOperationState::Completed
                    | AiToolOperationState::Failed
                    | AiToolOperationState::Cancelled
                    | AiToolOperationState::Interrupted
            ) {
                continue;
            }
            self.pending.remove(operation_id);
            self.pending_order
                .retain(|pending_id| pending_id != operation_id);
            let result = failed_result(
                &operation.tool_id,
                operation_id,
                Some(operation.project_identity),
                0,
                kernel_error(
                    "ai_tool.operation_context_invalidated",
                    "The Editor project context changed before the operation reached a terminal state.",
                    "Observe this terminal outcome and submit new work only against the active project.",
                ),
            );
            if let Some(operation) = self.operations.get_mut(operation_id) {
                transition_operation(
                    operation,
                    AiToolOperationState::Interrupted,
                    "context_invalidated",
                );
                operation.completed_at_epoch_ms = Some(now_epoch_ms());
                operation.result = Some(result);
            }
        }

        operation_ids
            .into_iter()
            .filter_map(|operation_id| {
                self.operations
                    .get(&operation_id)
                    .cloned()
                    .map(|operation| (operation_id, operation))
            })
            .collect()
    }

    pub fn cancel(
        &mut self,
        operation_id: &str,
        grant: &AiCapabilityGrant,
    ) -> Result<AiToolCancellationReceipt, AiToolKernelError> {
        self.cancel_inner(None, operation_id, grant)
    }

    pub fn cancel_durable(
        &mut self,
        session: &EditorSession,
        operation_id: &str,
        grant: &AiCapabilityGrant,
    ) -> Result<AiToolCancellationReceipt, AiToolKernelError> {
        self.cancel_inner(Some(session), operation_id, grant)
    }

    fn cancel_inner(
        &mut self,
        session: Option<&EditorSession>,
        operation_id: &str,
        grant: &AiCapabilityGrant,
    ) -> Result<AiToolCancellationReceipt, AiToolKernelError> {
        grant.validate()?;
        let operation = self.operations.get_mut(operation_id).ok_or_else(|| {
            kernel_error(
                "ai_tool.operation_unknown",
                "Operation id cannot be cancelled because it is unknown.",
                "Inspect the active project and use a recorded operation id.",
            )
        })?;
        if operation.grant_digest != grant.grant_digest {
            return Err(kernel_error(
                "ai_tool.cancel_grant_mismatch",
                "Cancellation grant does not own the operation.",
                "Use the exact grant that started the operation.",
            ));
        }
        let commit_started = operation.commit_started;
        let (status, code, signal_sent, terminal) = match operation.state {
            AiToolOperationState::Queued
            | AiToolOperationState::AwaitingUser
            | AiToolOperationState::Preflight
            | AiToolOperationState::Prepared
            | AiToolOperationState::Running
            | AiToolOperationState::Cancelling => {
                if let Some(pending) = self.pending.get(operation_id) {
                    pending.cancellation.request_cancel();
                }
                operation.cancel_signal_sent = true;
                transition_operation(operation, AiToolOperationState::Cancelling, "cancelling");
                (
                    AiToolCancellationStatus::Cancelled,
                    "ai_tool.operation_cancel_signal_sent",
                    true,
                    false,
                )
            }
            AiToolOperationState::Completed
            | AiToolOperationState::Failed
            | AiToolOperationState::Cancelled
            | AiToolOperationState::Interrupted => (
                AiToolCancellationStatus::AlreadyTerminal,
                "ai_tool.operation_already_terminal",
                false,
                true,
            ),
        };
        if let Some(session) = session {
            self.persist(session)?;
        }
        Ok(AiToolCancellationReceipt {
            schema_version: AI_TOOL_CANCELLATION_RECEIPT_SCHEMA_VERSION.to_string(),
            operation_id: operation_id.to_string(),
            grant_digest: grant.grant_digest.clone(),
            status,
            cancelled_at_epoch_ms: now_epoch_ms(),
            diagnostic_code: code.to_string(),
            signal_sent,
            child_termination_observed: false,
            commit_started,
            terminal,
        })
    }

    fn execute_inner(
        &mut self,
        session: &mut EditorSession,
        invocation: &AiToolInvocation,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        operation_id: &str,
        started: Instant,
        cancellation: &runtime_cli::BoundedChildProcessCancellation,
    ) -> AiToolResult {
        match (&*invocation.tool_id, &invocation.payload) {
            (TOOL_ID_PROJECT_MUTATE, AiToolInvocationPayload::BoundGoalMutation(bound)) => {
                if let Err(error) = crate::GoalMutationModule::revalidate(session, bound) {
                    return failed_result(
                        TOOL_ID_PROJECT_MUTATE,
                        operation_id,
                        Some(binding.project_id.clone()),
                        started.elapsed().as_millis() as u64,
                        goal_mutation_error(error),
                    );
                }
                self.execute_candidate(
                    TOOL_ID_PROJECT_MUTATE,
                    session,
                    &bound.candidate_input,
                    grant,
                    binding,
                    operation_id,
                    started,
                    cancellation,
                )
            }
            (TOOL_ID_PROJECT_MUTATE, AiToolInvocationPayload::Candidate(input)) => self
                .execute_candidate(
                    TOOL_ID_PROJECT_MUTATE,
                    session,
                    input,
                    grant,
                    binding,
                    operation_id,
                    started,
                    cancellation,
                ),
            (
                TOOL_ID_PROJECT_ROLLBACK,
                AiToolInvocationPayload::RollbackCandidate { receipt },
            ) => self.execute_rollback(session, receipt, grant, binding, operation_id, started),
            (TOOL_ID_PROJECT_PREVIEW, AiToolInvocationPayload::Preview) => {
                self.execute_preview(session, grant, binding, operation_id, started, cancellation)
            }
            (TOOL_ID_PROJECT_SEARCH, AiToolInvocationPayload::ProjectSearch(input)) => self
                .execute_observation(
                    TOOL_ID_PROJECT_SEARCH,
                    session,
                    binding,
                    operation_id,
                    started,
                    |index| index.search(input).map(ProjectObservationResult::Search),
                ),
            (TOOL_ID_PROJECT_READ_OBJECT, AiToolInvocationPayload::ProjectReadObject(input)) => {
                self.execute_observation(
                    TOOL_ID_PROJECT_READ_OBJECT,
                    session,
                    binding,
                    operation_id,
                    started,
                    |index| {
                        index
                            .read_object(input)
                            .map(ProjectObservationResult::Object)
                    },
                )
            }
            (TOOL_ID_PROJECT_REFERENCES, AiToolInvocationPayload::ProjectReferences(input)) => self
                .execute_observation(
                    TOOL_ID_PROJECT_REFERENCES,
                    session,
                    binding,
                    operation_id,
                    started,
                    |index| {
                        index
                            .references(input)
                            .map(ProjectObservationResult::References)
                    },
                ),
            (
                TOOL_ID_PROJECT_SOURCE_SYMBOLS,
                AiToolInvocationPayload::ProjectSourceSymbols(input),
            ) => self.execute_observation(
                TOOL_ID_PROJECT_SOURCE_SYMBOLS,
                session,
                binding,
                operation_id,
                started,
                |index| {
                    index
                        .source_symbols(input)
                        .map(ProjectObservationResult::SourceSymbols)
                },
            ),
            (TOOL_ID_PROJECT_DIAGNOSTICS, AiToolInvocationPayload::ProjectDiagnostics(input)) => {
                self.execute_observation(
                    TOOL_ID_PROJECT_DIAGNOSTICS,
                    session,
                    binding,
                    operation_id,
                    started,
                    |index| {
                        index
                            .diagnostics(input)
                            .map(ProjectObservationResult::Diagnostics)
                    },
                )
            }
            (TOOL_ID_EVIDENCE_READ, AiToolInvocationPayload::EvidenceRead(input)) => self
                .execute_observation(
                    TOOL_ID_EVIDENCE_READ,
                    session,
                    binding,
                    operation_id,
                    started,
                    |index| {
                        index
                            .read_evidence_input(input)
                            .map(ProjectObservationResult::Evidence)
                    },
                ),
            (TOOL_ID_UI_LOCATE, AiToolInvocationPayload::UiLocate(input)) => {
                match ProjectVisualDiagnostics::locate(session, input) {
                    Ok(result) => {
                        let mut completed = completed_result(
                            TOOL_ID_UI_LOCATE,
                            operation_id,
                            binding.project_id.clone(),
                            started.elapsed().as_millis() as u64,
                            Vec::new(),
                            AiToolOutput::UiLocated(result),
                            vec![
                                "Explain visibility for the intended stable candidate."
                                    .to_string(),
                            ],
                        );
                        if let Some(issue_bundle_ref) = &input.issue_bundle_ref {
                            completed.evidence_refs.push(issue_bundle_ref.clone());
                        }
                        completed
                    }
                    Err(error) => failed_result(
                        TOOL_ID_UI_LOCATE,
                        operation_id,
                        Some(binding.project_id.clone()),
                        started.elapsed().as_millis() as u64,
                        kernel_error(
                            "ai_tool.ui_locate_failed",
                            error,
                            "Narrow the visual name or text and retry.",
                        ),
                    ),
                }
            }
            (
                TOOL_ID_RUNTIME_CAPTURE_ISSUE,
                AiToolInvocationPayload::RuntimeCaptureIssue(input),
            ) => match ProjectVisualDiagnostics::capture_issue(session, operation_id, input) {
                Ok(bundle) => {
                    let issue_bundle_ref = bundle.issue_bundle_ref.clone();
                    let frame_evidence_ref = bundle.frame_evidence_ref.clone();
                    let mut completed = completed_result(
                        TOOL_ID_RUNTIME_CAPTURE_ISSUE,
                        operation_id,
                        binding.project_id.clone(),
                        started.elapsed().as_millis() as u64,
                        Vec::new(),
                        AiToolOutput::VisualIssueCaptured(bundle),
                        vec!["Locate stable AUI candidates against this issue bundle.".to_string()],
                    );
                    completed.evidence_refs.push(frame_evidence_ref);
                    completed.evidence_refs.push(issue_bundle_ref);
                    normalize_sorted(&mut completed.evidence_refs);
                    completed
                }
                Err(error) => failed_result(
                    TOOL_ID_RUNTIME_CAPTURE_ISSUE,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.runtime_capture_issue_failed",
                        error,
                        "Capture a fresh exact Preview frame and retry with only its evidence ref.",
                    ),
                ),
            },
            (
                TOOL_ID_UI_EXPLAIN_VISIBILITY,
                AiToolInvocationPayload::UiExplainVisibility(input),
            ) => match ProjectVisualDiagnostics::explain_visibility(session, input) {
                Ok(bundle) => {
                    let mut completed = completed_result(
                        TOOL_ID_UI_EXPLAIN_VISIBILITY,
                        operation_id,
                        binding.project_id.clone(),
                        started.elapsed().as_millis() as u64,
                        Vec::new(),
                        AiToolOutput::VisualIssue(bundle),
                        vec![
                            "Trace the owning bindings, actions, and source symbols.".to_string(),
                        ],
                    );
                    completed.evidence_refs.push(input.issue_bundle_ref.clone());
                    normalize_sorted(&mut completed.evidence_refs);
                    completed
                }
                Err(error) => failed_result(
                    TOOL_ID_UI_EXPLAIN_VISIBILITY,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.ui_visibility_failed",
                        error,
                        "Relocate the node against the current issue bundle and retry.",
                    ),
                ),
            },
            (
                TOOL_ID_PROJECT_TRACE_UI_OWNER,
                AiToolInvocationPayload::ProjectTraceUiOwner(input),
            ) => match ProjectVisualDiagnostics::trace_owner(session, input) {
                Ok(trace) => {
                    let mut completed = completed_result(
                        TOOL_ID_PROJECT_TRACE_UI_OWNER,
                        operation_id,
                        binding.project_id.clone(),
                        started.elapsed().as_millis() as u64,
                        Vec::new(),
                        AiToolOutput::UiOwnerTrace(trace),
                        vec![
                            "Inspect the returned project-owned symbols before proposing a Candidate."
                                .to_string(),
                        ],
                    );
                    if let Some(issue_bundle_ref) = &input.issue_bundle_ref {
                        completed.evidence_refs.push(issue_bundle_ref.clone());
                    }
                    normalize_sorted(&mut completed.evidence_refs);
                    completed
                }
                Err(error) => failed_result(
                    TOOL_ID_PROJECT_TRACE_UI_OWNER,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.ui_owner_trace_failed",
                        error,
                        "Repair stale AUI references or narrow the target node.",
                    ),
                ),
            },
            (
                TOOL_ID_PROJECT_BUILD_EXPORT,
                AiToolInvocationPayload::ProjectBuildExport(input),
            ) => match ProjectDeliveryTools::build_export(session, operation_id, input) {
                Ok(evidence) if evidence.report.status == crate::DesktopExportStatus::Success => {
                    completed_result(
                        TOOL_ID_PROJECT_BUILD_EXPORT,
                        operation_id,
                        binding.project_id.clone(),
                        started.elapsed().as_millis() as u64,
                        Vec::new(),
                        AiToolOutput::ProjectBuildExport(evidence),
                        vec!["Verify the frozen exported package through project.delivery_verify."
                            .to_string()],
                    )
                }
                Ok(evidence) => failed_result(
                    TOOL_ID_PROJECT_BUILD_EXPORT,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.build_export_failed",
                        evidence
                            .report
                            .diagnostics
                            .first()
                            .map(|diagnostic| diagnostic.message.clone())
                            .unwrap_or_else(|| "Desktop export failed.".to_string()),
                        "Inspect the desktop export report and fix the first diagnostic.",
                    ),
                ),
                Err(error) => failed_result(
                    TOOL_ID_PROJECT_BUILD_EXPORT,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.build_export_rejected",
                        error,
                        "Use the windows-dev profile and retry against the active project.",
                    ),
                ),
            },
            (
                TOOL_ID_PROJECT_DELIVERY_VERIFY,
                AiToolInvocationPayload::ProjectDeliveryVerify(input),
            ) => match ProjectDeliveryTools::verify_delivery(session, input) {
                Ok(evidence)
                    if evidence.report.status
                        == runtime_cli::ExportedPlayerProcessVerificationStatus::Passed =>
                {
                    completed_result(
                        TOOL_ID_PROJECT_DELIVERY_VERIFY,
                        operation_id,
                        binding.project_id.clone(),
                        started.elapsed().as_millis() as u64,
                        Vec::new(),
                        AiToolOutput::ProjectDeliveryVerify(evidence),
                        Vec::new(),
                    )
                }
                Ok(evidence) => failed_result(
                    TOOL_ID_PROJECT_DELIVERY_VERIFY,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.delivery_verify_failed",
                        evidence
                            .report
                            .diagnostics
                            .first()
                            .map(|diagnostic| diagnostic.message.clone())
                            .unwrap_or_else(|| "Delivery verification failed.".to_string()),
                        "Inspect the exported player verification report and retry after repair.",
                    ),
                ),
                Err(error) => failed_result(
                    TOOL_ID_PROJECT_DELIVERY_VERIFY,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    kernel_error(
                        "ai_tool.delivery_verify_rejected",
                        error,
                        "Use a package produced by project.build_export for this project.",
                    ),
                ),
            },
            _ => failed_result(
                &invocation.tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.invocation_tool_payload_mismatch",
                    "Tool id and invocation payload do not match a catalog entry.",
                    "Read the Tool Catalog and use the declared payload schema.",
                ),
            ),
        }
    }

    fn execute_observation<F>(
        &self,
        tool_id: &str,
        session: &EditorSession,
        binding: &crate::ProjectCandidateProjectBinding,
        operation_id: &str,
        started: Instant,
        observe: F,
    ) -> AiToolResult
    where
        F: FnOnce(&ProjectObservationIndex) -> Result<ProjectObservationResult, String>,
    {
        let result = ProjectObservationIndex::build(session).and_then(|index| observe(&index));
        match result {
            Ok(observation) => completed_result(
                tool_id,
                operation_id,
                binding.project_id.clone(),
                started.elapsed().as_millis() as u64,
                Vec::new(),
                AiToolOutput::ProjectObservation(observation),
                vec!["Continue from returned stable references or pagination token.".to_string()],
            ),
            Err(error) => failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.project_observation_failed",
                    error,
                    "Narrow the bounded query or repair the referenced project object.",
                ),
            ),
        }
    }

    fn execute_candidate(
        &mut self,
        tool_id: &str,
        session: &mut EditorSession,
        input: &AiCandidateToolInput,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        operation_id: &str,
        started: Instant,
        cancellation: &runtime_cli::BoundedChildProcessCancellation,
    ) -> AiToolResult {
        let profile = candidate_capability_profile(&input.envelope.payload);
        if let Err(error) = self.validate_mutation_grant(grant, binding, &profile) {
            return failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                error,
            );
        }
        if input.envelope.target_project_id != binding.project_id
            || input.envelope.expected_base_project_digest != binding.project_digest
        {
            return failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.candidate_binding_mismatch",
                    "Candidate does not target the current grant lineage revision.",
                    "Reprepare the Candidate from the current project binding.",
                ),
            );
        }

        let prepared = if let Some(source_file) = &input.source_file_path {
            ProjectCandidateEntry::prepare_with_source_file(
                session,
                ProjectCandidatePrepareRequest {
                    envelope: input.envelope.clone(),
                },
                source_file,
            )
        } else {
            ProjectCandidateEntry::prepare(
                session,
                ProjectCandidatePrepareRequest {
                    envelope: input.envelope.clone(),
                },
            )
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return failed_result(
                    tool_id,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                );
            }
        };
        let changed_paths_or_objects = changed_objects(&prepared.prepared_payload);
        let validation = match ProjectCandidateEntry::validate(
            session,
            &prepared,
            &ProjectCandidateValidationContext {
                controlled_source_patch: input.controlled_source_patch_validation.clone(),
                cancellation: Some(cancellation.clone()),
            },
        ) {
            Ok(validation) => validation,
            Err(error) => {
                return failed_result(
                    tool_id,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                );
            }
        };
        if validation.status != ProjectCandidateValidationStatus::Passed {
            return failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.candidate_validation_failed",
                    format!(
                        "Candidate validation failed: {}",
                        validation.diagnostics.join(", ")
                    ),
                    validation.next_actions.first().cloned().unwrap_or_else(|| {
                        "Inspect validation diagnostics and replan.".to_string()
                    }),
                ),
            );
        }
        if cancellation.is_cancelled() {
            return failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                kernel_error(
                    "ai_tool.operation_cancelled_before_commit",
                    "Candidate execution was cancelled after validation and before commit.",
                    "Observe the terminal cancellation receipt before submitting a replacement.",
                ),
            );
        }
        let approval = ProjectCandidateApproval {
            schema_version: PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION.to_string(),
            candidate_id: prepared.envelope.candidate_id.clone(),
            candidate_digest: prepared.candidate_digest.clone(),
            validation_digest: validation.validation_digest.clone(),
            approved_by: format!("capability-grant:{}", grant.grant_digest),
            allow_replace: grant.allow_delete,
        };
        if let Some(operation) = self.operations.get_mut(operation_id) {
            operation.commit_started = true;
        }
        if let Err(error) = self.persist(session) {
            return failed_result(
                tool_id,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                error,
            );
        }
        let receipt = match ProjectCandidateEntry::apply(session, prepared, validation, approval) {
            Ok(receipt) => receipt,
            Err(error) => {
                return failed_result(
                    tool_id,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                );
            }
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        let mut tool_receipt = AiToolMutationReceipt {
            schema_version: AI_TOOL_MUTATION_RECEIPT_SCHEMA_VERSION.to_string(),
            receipt_id: format!("receipt-{operation_id}"),
            operation_id: operation_id.to_string(),
            tool_id: tool_id.to_string(),
            tool_version: tool_version_for(tool_id).to_string(),
            grant_digest: grant.grant_digest.clone(),
            project_identity: binding.project_id.clone(),
            before_project_digest: receipt.before_project_digest.clone(),
            after_project_digest: receipt.applied_project_digest.clone(),
            changed_paths_or_objects,
            changed_domains: profile.domains.clone(),
            candidate_digest: receipt.candidate_digest.clone(),
            validation_digest: receipt.validation_digest.clone(),
            rollback_handle: receipt.candidate_id.clone(),
            candidate_receipt: receipt,
            duration_ms,
            external_cost_microunits: 0,
            receipt_digest: String::new(),
        };
        tool_receipt.receipt_digest =
            digest_serializable(&tool_receipt, "tool mutation receipt").unwrap_or_default();
        self.advance_lineage(
            grant,
            &tool_receipt.after_project_digest,
            duration_ms,
            &tool_receipt.receipt_digest,
        );
        completed_result(
            tool_id,
            operation_id,
            binding.project_id.clone(),
            duration_ms,
            profile.domains,
            AiToolOutput::CandidateApplied(tool_receipt),
            vec![
                "Run the smallest validation or Preview that proves the user-visible outcome."
                    .to_string(),
            ],
        )
    }

    fn execute_rollback(
        &mut self,
        session: &mut EditorSession,
        receipt: &ProjectCandidateApplyReceipt,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        operation_id: &str,
        started: Instant,
    ) -> AiToolResult {
        if let Err(error) = self.validate_rollback_grant(grant, binding, receipt) {
            return failed_result(
                TOOL_ID_PROJECT_ROLLBACK,
                operation_id,
                Some(binding.project_id.clone()),
                started.elapsed().as_millis() as u64,
                error,
            );
        }
        let rollback = match ProjectCandidateEntry::rollback(session, receipt) {
            Ok(receipt) => receipt,
            Err(error) => {
                return failed_result(
                    TOOL_ID_PROJECT_ROLLBACK,
                    operation_id,
                    Some(binding.project_id.clone()),
                    started.elapsed().as_millis() as u64,
                    candidate_entry_error(error),
                );
            }
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        let mut tool_receipt = AiToolRollbackReceipt {
            schema_version: AI_TOOL_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
            receipt_id: format!("rollback-{operation_id}"),
            operation_id: operation_id.to_string(),
            tool_id: TOOL_ID_PROJECT_ROLLBACK.to_string(),
            grant_digest: grant.grant_digest.clone(),
            project_identity: binding.project_id.clone(),
            replaced_project_digest: rollback.replaced_project_digest.clone(),
            restored_project_digest: rollback.restored_project_digest.clone(),
            candidate_rollback: rollback,
            duration_ms,
            receipt_digest: String::new(),
        };
        tool_receipt.receipt_digest =
            digest_serializable(&tool_receipt, "tool rollback receipt").unwrap_or_default();
        self.advance_rollback_lineage(
            grant,
            &tool_receipt.restored_project_digest,
            duration_ms,
            &tool_receipt.receipt_digest,
        );
        completed_result(
            TOOL_ID_PROJECT_ROLLBACK,
            operation_id,
            binding.project_id.clone(),
            duration_ms,
            vec!["rollback".to_string()],
            AiToolOutput::CandidateRolledBack(tool_receipt),
            vec!["Re-inspect the restored project before planning another mutation.".to_string()],
        )
    }

    fn execute_preview(
        &mut self,
        _session: &mut EditorSession,
        _grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        operation_id: &str,
        started: Instant,
        _cancellation: &runtime_cli::BoundedChildProcessCancellation,
    ) -> AiToolResult {
        failed_result(
            TOOL_ID_PROJECT_PREVIEW,
            operation_id,
            Some(binding.project_id.clone()),
            started.elapsed().as_millis() as u64,
            kernel_error(
                "ai_tool.preview_async_barrier_bypassed",
                "Preview execution bypassed the presented-frame operation barrier.",
                "Use start/observe/pump so Preview can await real-window frame evidence.",
            ),
        )
    }

    fn validate_tool_authorization(
        &self,
        invocation: &AiToolInvocation,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
    ) -> Result<(), AiToolKernelError> {
        match (&*invocation.tool_id, &invocation.payload) {
            (TOOL_ID_PROJECT_MUTATE, AiToolInvocationPayload::BoundGoalMutation(bound)) => {
                let profile = candidate_capability_profile(&bound.candidate_input.envelope.payload);
                self.validate_mutation_grant(grant, binding, &profile)
            }
            (TOOL_ID_PROJECT_MUTATE, AiToolInvocationPayload::Candidate(input)) => {
                let profile = candidate_capability_profile(&input.envelope.payload);
                self.validate_mutation_grant(grant, binding, &profile)
            }
            (TOOL_ID_PROJECT_ROLLBACK, AiToolInvocationPayload::RollbackCandidate { receipt }) => {
                self.validate_rollback_grant(grant, binding, receipt)
            }
            (TOOL_ID_PROJECT_PREVIEW, AiToolInvocationPayload::Preview) => {
                if grant.kind == AiCapabilityGrantKind::Read {
                    if grant.initial_base_digest != binding.project_digest {
                        return Err(kernel_error(
                            "ai_tool.read_grant_project_drifted",
                            "Read grant does not match the current project revision.",
                            "Inspect the project and issue a fresh ReadGrant.",
                        ));
                    }
                    Ok(())
                } else {
                    self.validate_lineage(grant, binding)
                }
            }
            (TOOL_ID_PROJECT_SEARCH, AiToolInvocationPayload::ProjectSearch(_))
            | (TOOL_ID_PROJECT_READ_OBJECT, AiToolInvocationPayload::ProjectReadObject(_))
            | (TOOL_ID_PROJECT_REFERENCES, AiToolInvocationPayload::ProjectReferences(_))
            | (TOOL_ID_PROJECT_SOURCE_SYMBOLS, AiToolInvocationPayload::ProjectSourceSymbols(_))
            | (TOOL_ID_PROJECT_DIAGNOSTICS, AiToolInvocationPayload::ProjectDiagnostics(_))
            | (TOOL_ID_EVIDENCE_READ, AiToolInvocationPayload::EvidenceRead(_))
            | (TOOL_ID_RUNTIME_CAPTURE_ISSUE, AiToolInvocationPayload::RuntimeCaptureIssue(_))
            | (TOOL_ID_UI_LOCATE, AiToolInvocationPayload::UiLocate(_))
            | (TOOL_ID_UI_EXPLAIN_VISIBILITY, AiToolInvocationPayload::UiExplainVisibility(_))
            | (TOOL_ID_PROJECT_TRACE_UI_OWNER, AiToolInvocationPayload::ProjectTraceUiOwner(_))
            | (TOOL_ID_PROJECT_BUILD_EXPORT, AiToolInvocationPayload::ProjectBuildExport(_))
            | (
                TOOL_ID_PROJECT_DELIVERY_VERIFY,
                AiToolInvocationPayload::ProjectDeliveryVerify(_),
            ) => {
                if grant.kind != AiCapabilityGrantKind::Read {
                    return Err(kernel_error(
                        "ai_tool.read_grant_required",
                        "Project observation tools require a project-bound Read grant.",
                        "Issue a fresh read grant for the current project digest.",
                    ));
                }
                if grant.initial_base_digest != binding.project_digest {
                    return Err(kernel_error(
                        "ai_tool.read_grant_project_drifted",
                        "Read grant does not match the current project revision.",
                        "Inspect the project and issue a fresh ReadGrant.",
                    ));
                }
                Ok(())
            }
            _ => Err(kernel_error(
                "ai_tool.invocation_tool_payload_mismatch",
                "Tool id and invocation payload do not match a catalog entry.",
                "Read the Tool Catalog and use the declared payload schema.",
            )),
        }
    }

    fn validate_mutation_grant(
        &self,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        profile: &CandidateCapabilityProfile,
    ) -> Result<(), AiToolKernelError> {
        if !matches!(
            grant.kind,
            AiCapabilityGrantKind::ScopedMutation | AiCapabilityGrantKind::Elevated
        ) {
            return Err(kernel_error(
                "ai_tool.mutation_grant_required",
                "Mutation tool requires a ScopedMutation or Elevated grant.",
                "Request user approval for the visible outcome and risk scope.",
            ));
        }
        if !grant
            .allowed_mutation_kinds
            .contains(&profile.mutation_kind)
        {
            return Err(kernel_error(
                "ai_tool.mutation_kind_not_granted",
                "Mutation kind is outside the CapabilityGrant.",
                "Use an allowed implementation or request a revised grant.",
            ));
        }
        if profile
            .domains
            .iter()
            .any(|domain| !grant.allowed_domains.contains(domain))
        {
            return Err(kernel_error(
                "ai_tool.domain_not_granted",
                "Candidate changes a domain outside the CapabilityGrant.",
                "Replan within the approved domains or request an expanded grant.",
            ));
        }
        if profile.requires_delete && !grant.allow_delete {
            return Err(kernel_error(
                "ai_tool.delete_not_granted",
                "Candidate contains delete/remove operations without delete permission.",
                "Request explicit delete authorization or choose a non-destructive implementation.",
            ));
        }
        if profile.requires_dependency_change && !grant.allow_dependency_change {
            return Err(kernel_error(
                "ai_tool.dependency_change_not_granted",
                "Candidate changes RuntimeModule dependencies without permission.",
                "Request explicit dependency-change authorization.",
            ));
        }
        self.validate_lineage(grant, binding)?;
        let mutation_count = self
            .lineages
            .get(&grant.grant_digest)
            .map_or(0, |lineage| lineage.mutation_count);
        if mutation_count >= grant.max_mutation_count {
            return Err(kernel_error(
                "ai_tool.mutation_budget_exhausted",
                "CapabilityGrant mutation count budget is exhausted.",
                "Review completed receipts and request a new bounded grant if more changes are needed.",
            ));
        }
        let consumed_time_ms = self
            .lineages
            .get(&grant.grant_digest)
            .map_or(0, |lineage| lineage.consumed_time_ms);
        if consumed_time_ms >= grant.time_budget_ms {
            return Err(kernel_error(
                "ai_tool.grant_time_budget_exhausted",
                "CapabilityGrant execution time budget is exhausted.",
                "Review progress and request a new time budget.",
            ));
        }
        Ok(())
    }

    fn validate_rollback_grant(
        &self,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
        receipt: &ProjectCandidateApplyReceipt,
    ) -> Result<(), AiToolKernelError> {
        grant.validate_rollback_integrity()?;
        if grant.project_identity != binding.project_id
            || receipt.project_id != binding.project_id
            || receipt.applied_project_digest != binding.project_digest
            || !grant
                .allowed_mutation_kinds
                .contains(&AiMutationKind::Rollback)
            || !grant.allowed_domains.contains(&"rollback".to_string())
        {
            return Err(kernel_error(
                "ai_tool.rollback_authority_mismatch",
                "Rollback receipt, project state, and durable Grant snapshot do not match.",
                "Use the exact mutation receipt in its owning project session.",
            ));
        }
        self.validate_lineage(grant, binding)
    }

    fn validate_lineage(
        &self,
        grant: &AiCapabilityGrant,
        binding: &crate::ProjectCandidateProjectBinding,
    ) -> Result<(), AiToolKernelError> {
        let expected = self
            .lineages
            .get(&grant.grant_digest)
            .map(|lineage| lineage.current_project_digest.as_str())
            .unwrap_or(grant.initial_base_digest.as_str());
        if expected != binding.project_digest {
            return Err(kernel_error(
                "ai_tool.grant_lineage_drifted",
                format!(
                    "Project digest '{}' is not the authorized lineage digest '{}'.",
                    binding.project_digest, expected
                ),
                "Inspect intervening changes and request a grant from the accepted current base.",
            ));
        }
        Ok(())
    }

    fn advance_lineage(
        &mut self,
        grant: &AiCapabilityGrant,
        current_project_digest: &str,
        duration_ms: u64,
        receipt_digest: &str,
    ) {
        let lineage = self
            .lineages
            .entry(grant.grant_digest.clone())
            .or_insert_with(|| new_lineage(grant));
        lineage.current_project_digest = current_project_digest.to_string();
        lineage.mutation_count = lineage.mutation_count.saturating_add(1);
        lineage.consumed_time_ms = lineage.consumed_time_ms.saturating_add(duration_ms);
        lineage.receipt_digests.push(receipt_digest.to_string());
    }

    fn advance_rollback_lineage(
        &mut self,
        grant: &AiCapabilityGrant,
        current_project_digest: &str,
        duration_ms: u64,
        receipt_digest: &str,
    ) {
        let lineage = self
            .lineages
            .entry(grant.grant_digest.clone())
            .or_insert_with(|| new_lineage(grant));
        lineage.current_project_digest = current_project_digest.to_string();
        lineage.consumed_time_ms = lineage.consumed_time_ms.saturating_add(duration_ms);
        lineage.receipt_digests.push(receipt_digest.to_string());
    }

    fn hydrate(
        &mut self,
        session: &EditorSession,
        project_identity: &str,
    ) -> Result<(), AiToolKernelError> {
        if self.loaded_project_identity.as_deref() == Some(project_identity) {
            return Ok(());
        }
        let project = session.active_project_session().ok_or_else(|| {
            kernel_error(
                "ai_tool.project_required",
                "Tool Kernel requires an active project.",
                "Open or create the target project.",
            )
        })?;
        let scope = project.write_scope();
        let journal = match scope.try_exists(TOOL_KERNEL_JOURNAL_PATH) {
            Ok(false) => AiToolKernelJournal::empty(project_identity),
            Ok(true) => {
                let bytes = scope
                    .read(TOOL_KERNEL_JOURNAL_PATH)
                    .map_err(write_scope_error)?;
                let journal: AiToolKernelJournal =
                    serde_json::from_slice(&bytes).map_err(|error| {
                        kernel_error(
                            "ai_tool.journal_parse_failed",
                            format!("Tool Kernel journal is invalid JSON: {error}"),
                            "Quarantine the invalid Library journal and recover from receipts.",
                        )
                    })?;
                journal.validate(project_identity)?;
                journal
            }
            Err(error) => return Err(write_scope_error(error)),
        };
        self.loaded_project_identity = Some(project_identity.to_string());
        self.lineages = journal.lineages;
        self.operations = journal.operations;
        self.pending.clear();
        self.pending_order.clear();
        let interrupted = self
            .operations
            .iter()
            .filter_map(|(operation_id, operation)| {
                matches!(
                    operation.state,
                    AiToolOperationState::Queued
                        | AiToolOperationState::Preflight
                        | AiToolOperationState::Prepared
                        | AiToolOperationState::Running
                        | AiToolOperationState::Cancelling
                )
                .then(|| operation_id.clone())
            })
            .collect::<Vec<_>>();
        for operation_id in interrupted {
            self.finish_interrupted_operation(session, &operation_id);
        }
        Ok(())
    }

    fn persist(&self, session: &EditorSession) -> Result<(), AiToolKernelError> {
        let project = session.active_project_session().ok_or_else(|| {
            kernel_error(
                "ai_tool.project_required",
                "Cannot persist Tool Kernel state without an active project.",
                "Open the target project before executing tools.",
            )
        })?;
        let mut journal = AiToolKernelJournal {
            schema_version: AI_TOOL_KERNEL_JOURNAL_SCHEMA_VERSION.to_string(),
            project_identity: project.manifest.project_id.clone(),
            lineages: self.lineages.clone(),
            operations: self.operations.clone(),
            journal_digest: String::new(),
        };
        journal.seal()?;
        let bytes = serde_json::to_vec_pretty(&journal).map_err(|error| {
            kernel_error(
                "ai_tool.journal_serialize_failed",
                format!("Failed to serialize Tool Kernel journal: {error}"),
                "Do not execute a mutation until journal serialization is repaired.",
            )
        })?;
        project
            .write_scope()
            .write_atomic(TOOL_KERNEL_JOURNAL_PATH, &bytes)
            .map_err(write_scope_error)?;
        Ok(())
    }
}

fn validate_invocation_preflight(
    invocation: &AiToolInvocation,
    grant: &AiCapabilityGrant,
    binding: &crate::ProjectCandidateProjectBinding,
) -> Result<(), AiToolKernelError> {
    if invocation.schema_version != AI_TOOL_INVOCATION_SCHEMA_VERSION {
        return Err(kernel_error(
            "ai_tool.invocation_schema_unsupported",
            "Tool invocation schema is unsupported.",
            "Regenerate the invocation using ai-tool-invocation.v1.",
        ));
    }
    if invocation.invocation_id.trim().is_empty() {
        return Err(kernel_error(
            "ai_tool.invocation_identity_invalid",
            "Tool invocation id is required.",
            "Generate a stable non-empty invocation id.",
        ));
    }
    if matches!(
        &invocation.payload,
        AiToolInvocationPayload::RollbackCandidate { .. }
    ) {
        grant.validate_rollback_integrity()?;
    } else {
        grant.validate()?;
    }
    if grant.project_identity != binding.project_id {
        return Err(kernel_error(
            "ai_tool.grant_project_mismatch",
            "CapabilityGrant targets a different project.",
            "Inspect the active project and request a correctly bound grant.",
        ));
    }
    if invocation.expected_project_digest != binding.project_digest {
        return Err(kernel_error(
            "ai_tool.invocation_project_drifted",
            "Project changed after the tool invocation was prepared.",
            "Inspect the current project and replan from the new digest.",
        ));
    }
    Ok(())
}

fn validate_candidate_prepare_preflight(
    session: &EditorSession,
    invocation: &AiToolInvocation,
) -> Result<(), AiToolKernelError> {
    let input = match &invocation.payload {
        AiToolInvocationPayload::Candidate(input) => input,
        AiToolInvocationPayload::BoundGoalMutation(bound) => {
            crate::GoalMutationModule::revalidate(session, bound).map_err(goal_mutation_error)?;
            &bound.candidate_input
        }
        _ => return Ok(()),
    };
    if !matches!(
        input.envelope.payload,
        ProjectCandidatePayload::ProjectPatch(_)
    ) {
        return Ok(());
    }
    let request = ProjectCandidatePrepareRequest {
        envelope: input.envelope.clone(),
    };
    if let Some(source_file) = &input.source_file_path {
        ProjectCandidateEntry::prepare_with_source_file(session, request, source_file)
    } else {
        ProjectCandidateEntry::prepare(session, request)
    }
    .map(|_| ())
    .map_err(candidate_entry_error)
}

fn prepare_project_owned_preview(
    session: &mut EditorSession,
    operation_id: &str,
) -> Result<PreparedProjectOwnedPreview, AiToolKernelError> {
    if session.scene_dirty() == Some(true) {
        return Err(kernel_error(
            "ai_tool.preview_dirty_scene_requires_save",
            "Project Preview cannot establish a stable project digest while the active Scene is dirty.",
            "Save the active Scene, inspect the new project digest, and start a fresh Preview operation.",
        ));
    }
    let binding =
        ProjectCandidateEntry::inspect_project_binding(session).map_err(candidate_entry_error)?;
    let expected = session
        .active_project_session()
        .ok_or_else(|| {
            kernel_error(
                "ai_tool.project_required",
                "Project Preview requires an active project.",
                "Open or create the target project.",
            )
        })?
        .manifest
        .runtime_module
        .clone();
    let source_kind = expected.resolved_source_kind();
    let linked = session
        .linked_project_runtimes
        .descriptor_for_module_id(&expected.module_id)
        .ok_or_else(|| {
            kernel_error(
                "ai_tool.preview_project_runtime_not_linked",
                format!(
                    "Project '{}' requires RuntimeModule '{}', which is not linked into this editor host.",
                    binding.project_id, expected.module_id
                ),
                "Build or launch an editor/player host that links the requested project RuntimeModule, then retry project.preview.",
            )
        })?;
    if linked.interface_version != expected.interface_version {
        return Err(kernel_error(
            "ai_tool.preview_project_runtime_not_linked",
            format!(
                "Project '{}' requires RuntimeModule '{}', but this editor host links '{}'.",
                binding.project_id, expected.module_id, linked.module_id
            ),
            "Build or launch the project-specific editor/player host, then retry project.preview.",
        ));
    }
    let linked_module_id = linked.module_id.clone();

    let result = session.execute_command(UiCommand {
        command_id: format!("ai-tool-preview-{operation_id}"),
        source: UiCommandSource::AiAssistant,
        request_id: format!("ai-tool-preview-request-{operation_id}"),
        payload: UiCommandPayload::Play,
    });
    if result.status != CommandStatus::Committed {
        return Err(kernel_error(
            "ai_tool.preview_failed",
            "Project-owned Preview did not commit.",
            "Inspect Editor Preview diagnostics and repair the first failed stage.",
        ));
    }
    let post_play_binding =
        ProjectCandidateEntry::inspect_project_binding(session).map_err(candidate_entry_error)?;
    if post_play_binding.project_id != binding.project_id
        || post_play_binding.project_digest != binding.project_digest
    {
        return Err(kernel_error(
            "ai_tool.preview_project_drifted_during_play_start",
            format!(
                "Project identity or digest changed while Preview started: before=({}, {}) after=({}, {}).",
                binding.project_id,
                binding.project_digest,
                post_play_binding.project_id,
                post_play_binding.project_digest,
            ),
            "Inspect the current project and start Preview again from the refreshed digest.",
        ));
    }
    let present = session
        .last_game_view_present_report()
        .cloned()
        .ok_or_else(|| {
            kernel_error(
                "ai_tool.preview_present_evidence_missing",
                "Preview committed without a GameView present report.",
                "Treat Preview as failed and repair the GameView evidence path.",
            )
        })?;
    let frame = session
        .last_game_view_runtime_frame()
        .cloned()
        .ok_or_else(|| {
            kernel_error(
                "ai_tool.preview_runtime_frame_missing",
                "Preview committed without an exact GameView runtime frame.",
                "Repair the project RuntimeModule frame producer before retrying Preview.",
            )
        })?;
    let reported_frame = present.last_frame.as_ref().ok_or_else(|| {
        kernel_error(
            "ai_tool.preview_present_frame_missing",
            "GameView present report did not retain the runtime frame selected for Preview.",
            "Repair GameView report retention before retrying Preview.",
        )
    })?;
    if reported_frame.session_id != frame.session_id
        || reported_frame.texture_id != frame.texture_id
        || reported_frame.frame_index != frame.frame_index
        || reported_frame.frame_hash != frame.frame_hash
        || present.session_id != frame.session_id
        || present.last_frame_hash.as_deref() != Some(frame.frame_hash.as_str())
    {
        return Err(kernel_error(
            "ai_tool.preview_present_frame_mismatch",
            "GameView present report and retained runtime frame do not identify the same frame.",
            "Discard the inconsistent Preview state and start a fresh Play session.",
        ));
    }
    if present.report_path.is_none() {
        return Err(kernel_error(
            "ai_tool.preview_present_report_path_missing",
            "Preview produced a GameView present report without a project-contained report path.",
            "Repair GameView report persistence before retrying Preview.",
        ));
    }
    let bind = present
        .project_runtime_bind_receipt
        .as_ref()
        .ok_or_else(|| {
            kernel_error(
                "ai_tool.preview_runtime_bind_evidence_missing",
                "Preview produced no ProjectRuntimeBindReceipt.",
                "Do not accept Preview without project runtime binding evidence.",
            )
        })?;
    if bind.project_id != binding.project_id
        || bind.module_id != expected.module_id
        || bind.interface_version != expected.interface_version
        || bind.status != "passed"
    {
        return Err(kernel_error(
            "ai_tool.preview_runtime_bind_mismatch",
            "Preview binding receipt does not match the active project RuntimeModule.",
            "Rebuild PreviewPackage and the linked project runtime from the same project revision.",
        ));
    }
    let bind_digest = digest_serializable(bind, "project runtime bind receipt")?;
    let ticket = ProjectPreviewFrameTicket {
        schema_version: PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.to_string(),
        project_identity: binding.project_id.clone(),
        expected_project_digest: binding.project_digest.clone(),
        game_view_session_id: frame.session_id,
        expected_texture_id: frame.texture_id,
        expected_frame_index: frame.frame_index,
        expected_runtime_frame_hash: frame.frame_hash,
    };
    session
        .begin_project_preview_frame_capture(ticket.clone())
        .map_err(|error| {
            kernel_error(
                error.code,
                error.message,
                "Clear the stale Preview frame capture and start a fresh operation.",
            )
        })?;
    Ok(PreparedProjectOwnedPreview {
        project_identity: binding.project_id,
        project_digest: binding.project_digest,
        runtime_source_kind: source_kind,
        expected_module_id: expected.module_id,
        linked_module_id,
        runtime_bind_receipt_digest: bind_digest,
        preview_package_report_path: session
            .last_editor_preview_package_report()
            .and_then(|report| report.report_path.clone()),
        present_report_path: present.report_path,
        ticket,
    })
}

fn completed_preview_result(
    prepared: &PreparedProjectOwnedPreview,
    frame: ProjectPreviewFrameEvidence,
    frame_evidence_ref: String,
    duration_ms: u64,
) -> AiToolResult {
    let mut result = completed_result(
        TOOL_ID_PROJECT_PREVIEW,
        &prepared.ticket.operation_id,
        prepared.project_identity.clone(),
        duration_ms,
        Vec::new(),
        AiToolOutput::Preview(AiToolPreviewEvidence {
            project_identity: prepared.project_identity.clone(),
            project_digest: prepared.project_digest.clone(),
            runtime_source_kind: prepared.runtime_source_kind.clone(),
            expected_module_id: prepared.expected_module_id.clone(),
            linked_module_id: prepared.linked_module_id.clone(),
            runtime_bind_receipt_digest: prepared.runtime_bind_receipt_digest.clone(),
            preview_package_report_path: prepared.preview_package_report_path.clone(),
            present_report_path: prepared.present_report_path.clone(),
            frame_evidence_ref: frame_evidence_ref.clone(),
            frame_evidence_digest: frame.evidence_digest.clone(),
            screenshot_ref: frame.screenshot_ref.clone(),
            screenshot_digest: frame.screenshot_digest.clone(),
            frame_index: frame.frame_index,
            frame_digest: frame.frame_digest.clone(),
            capture_kind: frame.capture_kind,
        }),
        vec![
            "Use the retained frame evidence reference with runtime.capture_issue before UI diagnosis."
                .to_string(),
        ],
    );
    result
        .facts
        .insert("frameEvidenceDigest".to_string(), frame.evidence_digest);
    result
        .facts
        .insert("frameDigest".to_string(), frame.frame_digest);
    result.evidence_refs.extend([
        frame_evidence_ref,
        frame.screenshot_ref,
        frame.present_report_ref,
    ]);
    result
}

pub(crate) fn execute_project_owned_preview(
    _session: &mut EditorSession,
    _operation_id: &str,
) -> Result<AiToolPreviewEvidence, AiToolKernelError> {
    Err(kernel_error(
        "ai_tool.preview_async_execution_required",
        "Project-owned Preview cannot complete through the synchronous compatibility workflow.",
        "Use the AI Capability Tool Kernel start/observe/pump workflow with a running native Editor window.",
    ))
}

#[derive(Debug)]
struct CandidateCapabilityProfile {
    mutation_kind: AiMutationKind,
    domains: Vec<String>,
    requires_delete: bool,
    requires_dependency_change: bool,
}

fn candidate_capability_profile(payload: &ProjectCandidatePayload) -> CandidateCapabilityProfile {
    match payload {
        ProjectCandidatePayload::ProjectPatch(patch) => {
            let mut domains = BTreeSet::new();
            let mut requires_delete = false;
            for operation in &patch.operations {
                let domain = match operation {
                    PatchOperation::Scene(_) => "scene",
                    PatchOperation::Input(_) => "input",
                    PatchOperation::Asset(_) => "asset",
                    PatchOperation::Prefab(_) => "prefab",
                    PatchOperation::Aui(_) => "aui",
                    PatchOperation::Rule(_) => "rule",
                    PatchOperation::Build(_) => "build",
                };
                domains.insert(domain.to_string());
                let kind = operation.kind().to_ascii_lowercase();
                requires_delete |= kind.contains("delete") || kind.contains("remove");
            }
            CandidateCapabilityProfile {
                mutation_kind: AiMutationKind::ProjectPatch,
                domains: domains.into_iter().collect(),
                requires_delete,
                requires_dependency_change: false,
            }
        }
        ProjectCandidatePayload::ControlledSourcePatch { request } => {
            let requires_delete = request.source_patch.operations.iter().any(|operation| {
                matches!(operation, ControlledSourcePatchOperation::Delete { .. })
            });
            let requires_dependency_change =
                request.source_patch.operations.iter().any(|operation| {
                    matches!(operation,
                    ControlledSourcePatchOperation::CreateOrReplace { path, .. }
                    | ControlledSourcePatchOperation::Delete { path }
                    if path.eq_ignore_ascii_case("RuntimeModule/Cargo.toml"))
                });
            CandidateCapabilityProfile {
                mutation_kind: AiMutationKind::ControlledSourcePatch,
                domains: vec!["runtime_module".to_string()],
                requires_delete,
                requires_dependency_change,
            }
        }
        ProjectCandidatePayload::AssetImport { request, .. } => CandidateCapabilityProfile {
            mutation_kind: AiMutationKind::AssetImport,
            domains: vec!["asset".to_string()],
            requires_delete: matches!(
                request.conflict_policy,
                AssetImportConflictPolicy::ReplaceMatching { .. }
            ),
            requires_dependency_change: false,
        },
    }
}

fn changed_objects(payload: &PreparedProjectCandidatePayload) -> Vec<String> {
    let mut values = match payload {
        PreparedProjectCandidatePayload::ProjectPatch { patch } => patch
            .operations
            .iter()
            .map(|operation| format!("{}:{}", operation.kind(), operation.target_summary()))
            .collect(),
        PreparedProjectCandidatePayload::ControlledSourcePatch { candidate } => {
            candidate.requested_paths.clone()
        }
        PreparedProjectCandidatePayload::AssetImport { candidate } => vec![
            candidate.record.descriptor_path.clone(),
            candidate.record.meta_path.clone(),
            candidate.record.source_path.clone(),
        ],
    };
    normalize_sorted(&mut values);
    values
}

fn decode_direct_value<T: DeserializeOwned>(
    tool_id: &str,
    direct_input: Value,
) -> Result<T, AiToolKernelError> {
    serde_json::from_value(direct_input).map_err(|error| {
        contract_error(
            "ai_tool.direct_input_decode_failed",
            format!("Direct input for '{tool_id}' could not be decoded: {error}"),
        )
    })
}

fn contract_error(code: impl Into<String>, message: impl Into<String>) -> AiToolKernelError {
    kernel_error(
        code,
        message,
        "Refresh the Tool Catalog and submit only the declared direct input fields.",
    )
}

fn validate_schema_value(schema: &Value, instance: &Value) -> Result<(), String> {
    validate_schema_node(schema, schema, instance, "$")
}

fn validate_schema_node(
    root: &Value,
    schema: &Value,
    instance: &Value,
    path: &str,
) -> Result<(), String> {
    let Some(object) = schema.as_object() else {
        return match schema.as_bool() {
            Some(true) => Ok(()),
            Some(false) => Err(format!("{path} is rejected by the schema")),
            None => Ok(()),
        };
    };

    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .ok_or_else(|| format!("{path} uses unsupported non-local schema ref '{reference}'"))?;
        let target = root
            .pointer(pointer)
            .ok_or_else(|| format!("{path} references missing schema '{reference}'"))?;
        validate_schema_node(root, target, instance, path)?;
    }

    if let Some(expected) = object.get("const") {
        if instance != expected {
            return Err(format!("{path} must equal {expected}"));
        }
    }
    if let Some(choices) = object.get("enum").and_then(Value::as_array) {
        if !choices.contains(instance) {
            return Err(format!("{path} is not one of the declared enum values"));
        }
    }
    if let Some(types) = object.get("type") {
        let matches = match types {
            Value::String(kind) => schema_type_matches(kind, instance),
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .any(|kind| schema_type_matches(kind, instance)),
            _ => false,
        };
        if !matches {
            return Err(format!("{path} has the wrong JSON type"));
        }
    }

    if let Some(branches) = object.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            validate_schema_node(root, branch, instance, path)?;
        }
    }
    if let Some(branches) = object.get("anyOf").and_then(Value::as_array) {
        if !branches
            .iter()
            .any(|branch| validate_schema_node(root, branch, instance, path).is_ok())
        {
            return Err(format!("{path} does not match any allowed schema branch"));
        }
    }
    if let Some(branches) = object.get("oneOf").and_then(Value::as_array) {
        let matching = branches
            .iter()
            .filter(|branch| validate_schema_node(root, branch, instance, path).is_ok())
            .count();
        if matching != 1 {
            return Err(format!(
                "{path} must match exactly one schema branch, matched {matching}"
            ));
        }
    }

    if let Some(value) = instance.as_object() {
        if let Some(required) = object.get("required").and_then(Value::as_array) {
            for field in required.iter().filter_map(Value::as_str) {
                if !value.contains_key(field) {
                    return Err(format!("{path} is missing required field '{field}'"));
                }
            }
        }
        let properties = object.get("properties").and_then(Value::as_object);
        for (field, child) in value {
            if let Some(property_schema) = properties.and_then(|items| items.get(field)) {
                validate_schema_node(root, property_schema, child, &format!("{path}/{field}"))?;
                continue;
            }
            match object.get("additionalProperties") {
                Some(Value::Bool(false)) => {
                    return Err(format!("{path} contains unknown field '{field}'"));
                }
                Some(additional_schema @ Value::Object(_)) => {
                    validate_schema_node(
                        root,
                        additional_schema,
                        child,
                        &format!("{path}/{field}"),
                    )?;
                }
                _ => {}
            }
        }
    }

    if let Some(items) = instance.as_array() {
        if let Some(minimum) = object.get("minItems").and_then(Value::as_u64) {
            if items.len() < minimum as usize {
                return Err(format!("{path} has fewer than {minimum} items"));
            }
        }
        if let Some(maximum) = object.get("maxItems").and_then(Value::as_u64) {
            if items.len() > maximum as usize {
                return Err(format!("{path} has more than {maximum} items"));
            }
        }
        if object.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
            for (index, item) in items.iter().enumerate() {
                if items[..index].contains(item) {
                    return Err(format!("{path} contains duplicate array items"));
                }
            }
        }
        if let Some(item_schema) = object.get("items") {
            for (index, item) in items.iter().enumerate() {
                validate_schema_node(root, item_schema, item, &format!("{path}/{index}"))?;
            }
        }
    }

    if let Some(text) = instance.as_str() {
        let length = text.chars().count() as u64;
        if object
            .get("minLength")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| length < minimum)
        {
            return Err(format!("{path} is shorter than the declared minimum"));
        }
        if object
            .get("maxLength")
            .and_then(Value::as_u64)
            .is_some_and(|maximum| length > maximum)
        {
            return Err(format!("{path} is longer than the declared maximum"));
        }
        if let Some(prefix_pattern) = object
            .get("pattern")
            .and_then(Value::as_str)
            .and_then(|pattern| pattern.strip_prefix('^'))
        {
            if !text.starts_with(prefix_pattern) {
                return Err(format!("{path} does not match the declared prefix"));
            }
        }
    }

    if let Some(number) = instance.as_f64() {
        if object
            .get("minimum")
            .and_then(Value::as_f64)
            .is_some_and(|minimum| number < minimum)
        {
            return Err(format!("{path} is below the declared minimum"));
        }
        if object
            .get("maximum")
            .and_then(Value::as_f64)
            .is_some_and(|maximum| number > maximum)
        {
            return Err(format!("{path} exceeds the declared maximum"));
        }
    }

    Ok(())
}

fn schema_type_matches(kind: &str, value: &Value) -> bool {
    match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "string" => value.is_string(),
        _ => false,
    }
}

fn builtin_descriptors() -> Vec<AiToolDescriptor> {
    vec![
        descriptor(
            TOOL_ID_PROJECT_CREATE,
            "Create and open one minimal project from the Editor launcher.",
            vec![AiToolSideEffect::ProjectMutation],
            Vec::new(),
            vec!["project".to_string()],
            AiToolDurationClass::Short,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_INSPECT,
            "Inspect active project identity, digest, runtime binding, and persisted tool facts.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_MUTATE,
            "Apply one goal-level project change while the engine owns project and Candidate facts.",
            vec![AiToolSideEffect::ProjectMutation],
            vec![AiToolCapability::MutateProject],
            vec!["invocation_dependent".to_string()],
            AiToolDurationClass::Short,
            true,
            false,
            true,
        ),
        descriptor(
            TOOL_ID_PROJECT_ROLLBACK,
            "Rollback one exact session-owned mutation through an opaque rollback reference.",
            vec![AiToolSideEffect::ProjectMutation],
            vec![AiToolCapability::MutateProject],
            vec!["rollback".to_string()],
            AiToolDurationClass::Short,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_PREVIEW,
            "Run project-owned Preview and prove the linked RuntimeModule binding.",
            vec![
                AiToolSideEffect::ProjectRead,
                AiToolSideEffect::GeneratedFiles,
                AiToolSideEffect::ProcessSpawn,
            ],
            vec![AiToolCapability::ReadProject],
            vec!["preview".to_string()],
            AiToolDurationClass::Long,
            false,
            true,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_SEARCH,
            "Search project-owned objects by name, text, type, binding, or action without knowing paths.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_READ_OBJECT,
            "Read one bounded project object through a stable object reference.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_REFERENCES,
            "Find stable project references to a symbol, binding, action, GUID, or value.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_SOURCE_SYMBOLS,
            "Find project-owned Rust source declarations without compiling the workspace.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_DIAGNOSTICS,
            "Search bounded project diagnostic and report objects.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_EVIDENCE_READ,
            "Read bounded trusted evidence referenced with project-evidence: under Library/Reports/ or Library/AiToolKernel/. This is not a project.preview frameEvidenceRef consumer; pass Preview evidence directly to runtime.capture_issue.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_RUNTIME_CAPTURE_ISSUE,
            "Capture a bounded runtime-to-AUI visual issue bundle for one stable UI node.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Short,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_UI_LOCATE,
            "Locate AUI nodes by visible name, authored text, or stable node id.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_UI_EXPLAIN_VISIBILITY,
            "Explain the first semantic visibility failure for one located AUI node.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Short,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_TRACE_UI_OWNER,
            "Trace an AUI node to project-owned bindings, actions, references, and source symbols.",
            vec![AiToolSideEffect::ProjectRead],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Instant,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_BUILD_EXPORT,
            "Build one frozen Windows development package into the isolated Gateway delivery root.",
            vec![
                AiToolSideEffect::ProjectRead,
                AiToolSideEffect::GeneratedFiles,
                AiToolSideEffect::ProcessSpawn,
            ],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Long,
            false,
            false,
            false,
        ),
        descriptor(
            TOOL_ID_PROJECT_DELIVERY_VERIFY,
            "Launch and verify an existing Gateway-built package without rebuilding or mutating project source.",
            vec![AiToolSideEffect::ProjectRead, AiToolSideEffect::ProcessSpawn],
            vec![AiToolCapability::ReadProject],
            Vec::new(),
            AiToolDurationClass::Long,
            false,
            true,
            false,
        ),
    ]
}

fn descriptor(
    tool_id: &str,
    summary: &str,
    side_effects: Vec<AiToolSideEffect>,
    required_capabilities: Vec<AiToolCapability>,
    changed_domains: Vec<String>,
    expected_duration_class: AiToolDurationClass,
    supports_dry_run: bool,
    supports_cancellation: bool,
    supports_rollback: bool,
) -> AiToolDescriptor {
    let (mut input_schema, output_schema, mut minimal_input_example) =
        direct_tool_contract(tool_id);
    hide_internal_direct_input_schema_version(
        tool_id,
        &mut input_schema,
        &mut minimal_input_example,
    );
    AiToolDescriptor {
        schema_version: AI_TOOL_DESCRIPTOR_SCHEMA_VERSION.to_string(),
        tool_id: tool_id.to_string(),
        tool_version: tool_version_for(tool_id).to_string(),
        summary: summary.to_string(),
        input_schema,
        output_schema,
        minimal_input_example,
        side_effects,
        required_capabilities,
        changed_domains,
        cost_class: AiToolCostClass::LocalCompute,
        expected_duration_class,
        supports_dry_run,
        supports_cancellation,
        supports_rollback,
        diagnostic_codes: vec![
            "ai_tool.grant_project_mismatch".to_string(),
            "ai_tool.grant_lineage_drifted".to_string(),
            "ai_tool.invocation_project_drifted".to_string(),
        ],
        idempotency_class: "invocation_id_exact_replay".to_string(),
        preconditions: if tool_id == TOOL_ID_PROJECT_CREATE {
            vec!["launcher_context".to_string()]
        } else {
            vec!["active_project".to_string()]
        },
        progress_event_schema: operation_event_schema(),
        completion_evidence: vec!["structured_tool_result".to_string()],
    }
}

fn tool_version_for(_tool_id: &str) -> &'static str {
    AI_TOOL_IMPLEMENTATION_VERSION_V1
}

fn internal_direct_input_schema_version(tool_id: &str) -> Option<&'static str> {
    match tool_id {
        TOOL_ID_PROJECT_MUTATE => Some(crate::EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION),
        TOOL_ID_PROJECT_INSPECT => Some(AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION),
        TOOL_ID_PROJECT_SEARCH
        | TOOL_ID_PROJECT_READ_OBJECT
        | TOOL_ID_PROJECT_REFERENCES
        | TOOL_ID_PROJECT_SOURCE_SYMBOLS
        | TOOL_ID_PROJECT_DIAGNOSTICS
        | TOOL_ID_EVIDENCE_READ => Some(crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION),
        TOOL_ID_RUNTIME_CAPTURE_ISSUE
        | TOOL_ID_UI_LOCATE
        | TOOL_ID_UI_EXPLAIN_VISIBILITY
        | TOOL_ID_PROJECT_TRACE_UI_OWNER => Some(crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION),
        TOOL_ID_PROJECT_BUILD_EXPORT | TOOL_ID_PROJECT_DELIVERY_VERIFY => {
            Some(crate::PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION)
        }
        _ => None,
    }
}

fn hide_internal_direct_input_schema_version(
    tool_id: &str,
    input_schema: &mut Value,
    minimal_input_example: &mut Value,
) {
    if internal_direct_input_schema_version(tool_id).is_none() {
        return;
    }
    let schema = input_schema
        .as_object_mut()
        .expect("direct tool input schema must be an object");
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|field| field.as_str() != Some("schemaVersion"));
    }
    schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("direct tool input schema properties must be an object")
        .remove("schemaVersion");
    minimal_input_example
        .as_object_mut()
        .expect("minimal direct tool input example must be an object")
        .remove("schemaVersion");
}

fn direct_tool_contract(tool_id: &str) -> (Value, Value, Value) {
    match tool_id {
        TOOL_ID_PROJECT_CREATE => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["requestedProjectRoot", "projectName"],
                "properties": {
                    "requestedProjectRoot": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 32767
                    },
                    "projectName": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    }
                }
            }),
            project_create_result_schema(),
            json!({
                "requestedProjectRoot": "C:/Projects/NewGame",
                "projectName": "NewGame"
            }),
        ),
        TOOL_ID_PROJECT_INSPECT => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "kind"],
                "properties": {
                    "schemaVersion": {"const": AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION},
                    "kind": {
                        "oneOf": [
                            {"const": "project"},
                            {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["grant_lineage"],
                                "properties": {
                                    "grant_lineage": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["grant_digest"],
                                        "properties": {
                                            "grant_digest": {"type": "string", "minLength": 1, "maxLength": 128}
                                        }
                                    }
                                }
                            }
                        ]
                    }
                }
            }),
            inspect_result_schema(),
            json!({
                "schemaVersion": AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION,
                "kind": "project"
            }),
        ),
        TOOL_ID_PROJECT_MUTATE => (
            goal_mutation_input_schema(),
            tool_result_schema("candidate_applied"),
            goal_mutation_input_example(),
        ),
        TOOL_ID_PROJECT_ROLLBACK => (
            external_rollback_input_schema(),
            tool_result_schema("candidate_rolled_back"),
            json!({
                "schemaVersion": EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
                "rollbackRef": "rbk_opaque"
            }),
        ),
        TOOL_ID_PROJECT_PREVIEW => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
            tool_result_schema("preview"),
            json!({}),
        ),
        TOOL_ID_PROJECT_SEARCH => observation_contract(
            &[
                "schemaVersion",
                "query",
                "kinds",
                "continuationToken",
                "pageSize",
            ],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "query": {"type": "string", "minLength": 1, "maxLength": 256},
                "kinds": {
                    "type": "array",
                    "maxItems": 32,
                    "uniqueItems": true,
                    "items": {"type": "string", "minLength": 1, "maxLength": 64}
                },
                "continuationToken": {"type": ["string", "null"], "maxLength": 256},
                "pageSize": {"type": "integer", "minimum": 1, "maximum": 100}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "query": "start button",
                "kinds": ["aui"],
                "continuationToken": null,
                "pageSize": 25
            }),
        ),
        TOOL_ID_PROJECT_READ_OBJECT => observation_contract(
            &["schemaVersion", "objectRef", "maxBytes"],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "objectRef": {"type": "string", "minLength": 1, "maxLength": 1024},
                "maxBytes": {"type": "integer", "minimum": 1, "maximum": 262144}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "objectRef": "project-object:AUI/main.aui.json",
                "maxBytes": 65536
            }),
        ),
        TOOL_ID_PROJECT_REFERENCES => observation_contract(
            &[
                "schemaVersion",
                "symbolOrValue",
                "continuationToken",
                "pageSize",
            ],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "symbolOrValue": {"type": "string", "minLength": 1, "maxLength": 512},
                "continuationToken": {"type": ["string", "null"], "maxLength": 256},
                "pageSize": {"type": "integer", "minimum": 1, "maximum": 100}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "symbolOrValue": "action.start_game",
                "continuationToken": null,
                "pageSize": 25
            }),
        ),
        TOOL_ID_PROJECT_SOURCE_SYMBOLS => observation_contract(
            &["schemaVersion", "query", "continuationToken", "pageSize"],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "query": {"type": "string", "minLength": 1, "maxLength": 256},
                "continuationToken": {"type": ["string", "null"], "maxLength": 256},
                "pageSize": {"type": "integer", "minimum": 1, "maximum": 100}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "query": "StartGame",
                "continuationToken": null,
                "pageSize": 25
            }),
        ),
        TOOL_ID_PROJECT_DIAGNOSTICS => observation_contract(
            &[
                "schemaVersion",
                "codeOrText",
                "continuationToken",
                "pageSize",
            ],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "codeOrText": {"type": ["string", "null"], "maxLength": 256},
                "continuationToken": {"type": ["string", "null"], "maxLength": 256},
                "pageSize": {"type": "integer", "minimum": 1, "maximum": 100}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "codeOrText": "visibility",
                "continuationToken": null,
                "pageSize": 25
            }),
        ),
        TOOL_ID_EVIDENCE_READ => observation_contract(
            &["schemaVersion", "evidenceRef", "maxBytes"],
            json!({
                "schemaVersion": {"const": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION},
                "evidenceRef": {
                    "description": "A project-evidence: reference under Library/Reports/ or Library/AiToolKernel/; this is not a Preview frameEvidenceRef. Use runtime.capture_issue for project.preview visual evidence.",
                    "oneOf": [
                        {
                            "type": "string",
                            "minLength": 34,
                            "maxLength": 1024,
                            "pattern": "^project-evidence:Library/Reports/"
                        },
                        {
                            "type": "string",
                            "minLength": 39,
                            "maxLength": 1024,
                            "pattern": "^project-evidence:Library/AiToolKernel/"
                        }
                    ]
                },
                "maxBytes": {"type": "integer", "minimum": 1, "maximum": 262144}
            }),
            json!({
                "schemaVersion": crate::PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION,
                "evidenceRef": "project-evidence:Library/Reports/preview.json",
                "maxBytes": 65536
            }),
        ),
        TOOL_ID_RUNTIME_CAPTURE_ISSUE => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "frameEvidenceRef"],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION},
                    "frameEvidenceRef": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "pattern": format!("^{}/", crate::PROJECT_PREVIEW_EVIDENCE_ROOT)
                    },
                    "symptom": {"type": ["string", "null"], "minLength": 1, "maxLength": 1024}
                }
            }),
            tool_result_schema("visual_issue"),
            json!({
                "schemaVersion": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION,
                "frameEvidenceRef": "Library/AiCapability/Preview/tool-op-example/frame-evidence.json"
            }),
        ),
        TOOL_ID_UI_EXPLAIN_VISIBILITY => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "documentPath", "nodeId", "issueBundleRef"],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION},
                    "documentPath": {"type": "string", "minLength": 1, "maxLength": 512},
                    "nodeId": {"type": "string", "minLength": 1, "maxLength": 256},
                    "issueBundleRef": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "pattern": format!("^{}/", crate::PROJECT_VISUAL_ISSUE_ROOT)
                    }
                }
            }),
            tool_result_schema("visual_issue"),
            json!({
                "schemaVersion": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION,
                "documentPath": "AUI/main-menu.aui.json",
                "nodeId": "start-game",
                "issueBundleRef": "Library/AiCapability/Visual/tool-op-example/issue-bundle.json"
            }),
        ),
        TOOL_ID_UI_LOCATE => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "query"],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION},
                    "query": {"type": "string", "minLength": 1, "maxLength": 256},
                    "issueBundleRef": {
                        "type": ["string", "null"],
                        "minLength": 1,
                        "maxLength": 1024,
                        "pattern": format!("^{}/", crate::PROJECT_VISUAL_ISSUE_ROOT)
                    }
                }
            }),
            tool_result_schema("ui_located"),
            json!({
                "schemaVersion": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION,
                "query": "Start Game"
            }),
        ),
        TOOL_ID_PROJECT_TRACE_UI_OWNER => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "documentPath", "nodeId"],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION},
                    "documentPath": {"type": "string", "minLength": 1, "maxLength": 512},
                    "nodeId": {"type": "string", "minLength": 1, "maxLength": 256},
                    "issueBundleRef": {
                        "type": ["string", "null"],
                        "minLength": 1,
                        "maxLength": 1024,
                        "pattern": format!("^{}/", crate::PROJECT_VISUAL_ISSUE_ROOT)
                    }
                }
            }),
            tool_result_schema("ui_owner_trace"),
            json!({
                "schemaVersion": crate::PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION,
                "documentPath": "AUI/main-menu.aui.json",
                "nodeId": "start-game"
            }),
        ),
        TOOL_ID_PROJECT_BUILD_EXPORT => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["schemaVersion", "profile"],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION},
                    "profile": {"const": "windows-dev"}
                }
            }),
            tool_result_schema("project_build_export"),
            json!({
                "schemaVersion": crate::PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION,
                "profile": "windows-dev"
            }),
        ),
        TOOL_ID_PROJECT_DELIVERY_VERIFY => (
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "schemaVersion", "packageDir", "mode", "timeoutMs", "frameLimit",
                    "screenshot"
                ],
                "properties": {
                    "schemaVersion": {"const": crate::PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION},
                    "packageDir": {"type": "string", "pattern": "^Library/AiCapability/Deliveries/"},
                    "mode": {"enum": ["headless", "windowed"]},
                    "timeoutMs": {"type": "integer", "minimum": 1, "maximum": 120000},
                    "frameLimit": {"type": "integer", "minimum": 1, "maximum": 600},
                    "screenshot": {"type": "boolean"}
                }
            }),
            tool_result_schema("project_delivery_verify"),
            json!({
                "schemaVersion": crate::PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION,
                "packageDir": "Library/AiCapability/Deliveries/tool-op-example/Windows/dev",
                "mode": "headless",
                "timeoutMs": 30000,
                "frameLimit": 3,
                "screenshot": false
            }),
        ),
        _ => (
            json!({"type": "object", "additionalProperties": false}),
            json!({"type": "object", "additionalProperties": false}),
            json!({}),
        ),
    }
}

fn observation_contract(
    required: &[&str],
    properties: Value,
    example: Value,
) -> (Value, Value, Value) {
    (
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false,
            "required": required,
            "properties": properties
        }),
        tool_result_schema("project_observation"),
        example,
    )
}

fn goal_mutation_input_schema() -> Value {
    let mut project_patch_schema = crate::project_patch_json_schema();
    let project_patch_definitions = project_patch_schema
        .as_object_mut()
        .and_then(|schema| schema.remove("$defs"))
        .unwrap_or_else(|| json!({}));
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": project_patch_definitions,
        "type": "object",
        "additionalProperties": false,
        "required": ["schemaVersion", "goal", "change"],
        "properties": {
            "schemaVersion": {
                "const": crate::EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION
            },
            "goal": {
                "type": "object",
                "additionalProperties": false,
                "required": ["outcome"],
                "properties": {
                    "outcome": {"type": "string", "minLength": 1, "maxLength": 2048}
                }
            },
            "change": {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "payload"],
                "properties": {
                    "kind": {"const": "project_patch"},
                    "payload": project_patch_schema
                }
            }
        }
    })
}

fn goal_mutation_input_example() -> Value {
    json!({
        "goal": {
            "outcome": "Add a player jump action."
        },
        "change": {
            "kind": "project_patch",
            "payload": {
                "schemaVersion": crate::PROJECT_PATCH_SCHEMA_VERSION,
                "patchId": "player-jump",
                "title": "Player jump",
                "source": "AiAssistant",
                "intentSummary": "",
                "targetProjectRoot": null,
                "requiredCapabilities": [],
                "operations": [],
                "expectedOutcome": "",
                "riskLevel": "Low",
                "createdAt": "0"
            }
        }
    })
}

fn inspect_result_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["schemaVersion", "payload"],
        "properties": {
            "schemaVersion": {"const": AI_TOOL_INSPECT_RESULT_SCHEMA_VERSION},
            "payload": {"type": "object"}
        }
    })
}

fn tool_result_schema(output_kind: &str) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion", "status", "toolId", "toolVersion", "operationId",
            "projectIdentity", "facts", "diagnostics", "suggestedNextActions",
            "changedDomains", "output", "evidenceRefs", "durationMs",
            "externalCostMicrounits"
        ],
        "properties": {
            "schemaVersion": {"const": AI_TOOL_RESULT_SCHEMA_VERSION},
            "status": {"enum": ["completed", "failed"]},
            "toolId": {"type": "string", "minLength": 1},
            "toolVersion": {"const": AI_TOOL_IMPLEMENTATION_VERSION_V1},
            "operationId": {"type": "string", "minLength": 1},
            "projectIdentity": {"type": ["string", "null"]},
            "facts": {"type": "object"},
            "diagnostics": {"type": "array"},
            "suggestedNextActions": {"type": "array", "items": {"type": "string"}},
            "changedDomains": {"type": "array", "items": {"type": "string"}},
            "output": {
                "oneOf": [
                    {"type": "null"},
                    {
                        "type": "object",
                        "required": ["outputKind", "output"],
                        "properties": {"outputKind": {"const": output_kind}, "output": {"type": "object"}}
                    }
                ]
            },
            "rollbackRef": {"type": "string", "minLength": 8, "maxLength": 128},
            "evidenceRefs": {"type": "array", "items": {"type": "string"}},
            "durationMs": {"type": "integer", "minimum": 0},
            "externalCostMicrounits": {"type": "integer", "minimum": 0}
        }
    })
}

fn external_rollback_input_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["schemaVersion", "rollbackRef"],
        "properties": {
            "schemaVersion": {"const": EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION},
            "rollbackRef": {
                "type": "string",
                "minLength": 8,
                "maxLength": 128
            }
        }
    })
}

fn validate_external_rollback_ref(rollback_ref: &str) -> Result<(), AiToolKernelError> {
    let suffix = rollback_ref.strip_prefix("rbk_").ok_or_else(|| {
        contract_error(
            "ai_tool.rollback_ref_invalid",
            "rollbackRef must use the opaque rbk_ reference format.",
        )
    })?;
    if suffix.len() < 4
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(contract_error(
            "ai_tool.rollback_ref_invalid",
            "rollbackRef contains invalid or insufficient opaque reference characters.",
        ));
    }
    Ok(())
}

fn project_create_result_schema() -> Value {
    let mut schema = tool_result_schema("project_created");
    schema["properties"]["facts"] = json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "status", "receiptId", "requestedProjectRoot", "canonicalProjectRoot",
            "projectName", "projectIdentity", "projectDigest", "readGeneration",
            "openedInEditor", "replayed"
        ],
        "properties": {
            "status": {"const": "created"},
            "receiptId": {"type": "string", "minLength": 1},
            "requestedProjectRoot": {"type": "string", "minLength": 1},
            "canonicalProjectRoot": {"type": "string", "minLength": 1},
            "projectName": {"type": "string", "minLength": 1},
            "projectIdentity": {"type": "string", "minLength": 1},
            "projectDigest": {"type": "string", "minLength": 1},
            "readGeneration": {"type": "string", "pattern": "^[1-9][0-9]*$"},
            "openedInEditor": {"const": "true"},
            "replayed": {"enum": ["true", "false"]}
        }
    });
    schema
}

fn operation_event_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schemaVersion", "operationId", "invocationId", "invocationDigest", "toolId",
            "grantDigest", "projectIdentity", "state", "stage", "startedAtEpochMs",
            "completedAtEpochMs", "result"
        ],
        "properties": {
            "schemaVersion": {"const": AI_TOOL_OPERATION_SCHEMA_VERSION},
            "operationId": {"type": "string", "minLength": 1},
            "invocationId": {"type": "string", "minLength": 1},
            "invocationDigest": {"type": "string", "minLength": 1},
            "toolId": {"type": "string", "minLength": 1},
            "grantDigest": {"type": "string", "minLength": 1},
            "projectIdentity": {"type": "string", "minLength": 1},
            "state": {"enum": ["running", "completed", "failed", "cancelled"]},
            "stage": {"type": "string", "minLength": 1},
            "startedAtEpochMs": {"type": "integer", "minimum": 0},
            "completedAtEpochMs": {"type": ["integer", "null"], "minimum": 0},
            "result": {"type": ["object", "null"]}
        }
    })
}

fn operation_id_for(invocation_id: &str, grant_digest: &str) -> String {
    format!(
        "tool-op-{}",
        sha256_prefixed(format!("{invocation_id}|{grant_digest}").as_bytes())
            .trim_start_matches("sha256:")
            .chars()
            .take(24)
            .collect::<String>()
    )
}

fn accepted_from(operation: &AiToolOperationSnapshot) -> AiToolAccepted {
    AiToolAccepted {
        schema_version: AI_TOOL_ACCEPTED_SCHEMA_VERSION.to_string(),
        operation_id: operation.operation_id.clone(),
        invocation_id: operation.invocation_id.clone(),
        tool_id: operation.tool_id.clone(),
        project_identity: operation.project_identity.clone(),
        state: operation.state,
        accepted_at_epoch_ms: operation.started_at_epoch_ms,
    }
}

fn transition_operation(
    operation: &mut AiToolOperationSnapshot,
    state: AiToolOperationState,
    stage: &str,
) {
    operation.state = state;
    operation.stage = stage.to_string();
    operation.transitions.push(AiToolOperationTransition {
        state,
        stage: stage.to_string(),
        at_epoch_ms: now_epoch_ms(),
    });
}

fn completed_result(
    tool_id: &str,
    operation_id: &str,
    project_identity: String,
    duration_ms: u64,
    changed_domains: Vec<String>,
    output: AiToolOutput,
    suggested_next_actions: Vec<String>,
) -> AiToolResult {
    let mut facts = BTreeMap::new();
    facts.insert("projectIdentity".to_string(), project_identity.clone());
    AiToolResult {
        schema_version: AI_TOOL_RESULT_SCHEMA_VERSION.to_string(),
        status: AiToolExecutionStatus::Completed,
        tool_id: tool_id.to_string(),
        tool_version: tool_version_for(tool_id).to_string(),
        operation_id: operation_id.to_string(),
        project_identity: Some(project_identity),
        facts,
        diagnostics: Vec::new(),
        suggested_next_actions,
        changed_domains,
        output: Some(output),
        rollback_ref: None,
        evidence_refs: vec![TOOL_KERNEL_JOURNAL_PATH.to_string()],
        duration_ms,
        external_cost_microunits: 0,
    }
}

fn project_create_completed_result(
    operation_id: &str,
    receipt: ProjectCreateToolReceipt,
    duration_ms: u64,
) -> AiToolResult {
    let mut facts = BTreeMap::new();
    facts.insert("status".to_string(), receipt.status);
    facts.insert("receiptId".to_string(), receipt.receipt_id);
    facts.insert(
        "requestedProjectRoot".to_string(),
        receipt.requested_project_root,
    );
    facts.insert(
        "canonicalProjectRoot".to_string(),
        receipt.canonical_project_root,
    );
    facts.insert("projectName".to_string(), receipt.project_name);
    facts.insert(
        "projectIdentity".to_string(),
        receipt.project_identity.clone(),
    );
    facts.insert("projectDigest".to_string(), receipt.project_digest);
    facts.insert(
        "readGeneration".to_string(),
        receipt.read_generation.to_string(),
    );
    facts.insert(
        "openedInEditor".to_string(),
        receipt.opened_in_editor.to_string(),
    );
    facts.insert("replayed".to_string(), receipt.replayed.to_string());
    AiToolResult {
        schema_version: AI_TOOL_RESULT_SCHEMA_VERSION.to_string(),
        status: AiToolExecutionStatus::Completed,
        tool_id: TOOL_ID_PROJECT_CREATE.to_string(),
        tool_version: tool_version_for(TOOL_ID_PROJECT_CREATE).to_string(),
        operation_id: operation_id.to_string(),
        project_identity: Some(receipt.project_identity),
        facts,
        diagnostics: Vec::new(),
        suggested_next_actions: Vec::new(),
        changed_domains: vec!["project".to_string()],
        output: None,
        rollback_ref: None,
        evidence_refs: Vec::new(),
        duration_ms,
        external_cost_microunits: 0,
    }
}

fn failed_result(
    tool_id: &str,
    operation_id: &str,
    project_identity: Option<String>,
    duration_ms: u64,
    error: AiToolKernelError,
) -> AiToolResult {
    let next_action = error.next_action.clone();
    AiToolResult {
        schema_version: AI_TOOL_RESULT_SCHEMA_VERSION.to_string(),
        status: AiToolExecutionStatus::Failed,
        tool_id: tool_id.to_string(),
        tool_version: tool_version_for(tool_id).to_string(),
        operation_id: operation_id.to_string(),
        project_identity,
        facts: BTreeMap::new(),
        diagnostics: vec![diagnostic_from_error(error)],
        suggested_next_actions: vec![next_action],
        changed_domains: Vec::new(),
        output: None,
        rollback_ref: None,
        evidence_refs: Vec::new(),
        duration_ms,
        external_cost_microunits: 0,
    }
}

fn diagnostic_from_error(error: AiToolKernelError) -> AiToolDiagnostic {
    AiToolDiagnostic {
        severity: AiToolDiagnosticSeverity::Error,
        code: error.code,
        message: error.message,
        next_action: error.next_action,
    }
}

fn new_lineage(grant: &AiCapabilityGrant) -> AiGrantLineage {
    AiGrantLineage {
        grant_digest: grant.grant_digest.clone(),
        current_project_digest: grant.initial_base_digest.clone(),
        mutation_count: 0,
        consumed_time_ms: 0,
        consumed_external_cost_microunits: 0,
        receipt_digests: Vec::new(),
    }
}

fn normalize_sorted(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}

fn digest_serializable<T: Serialize>(value: &T, role: &str) -> Result<String, AiToolKernelError> {
    let value = serde_json::to_value(value).map_err(|error| {
        kernel_error(
            "ai_tool.digest_failed",
            format!("Failed to serialize {role}: {error}"),
            "Use only canonical serializable tool data.",
        )
    })?;
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            kernel_error(
                "ai_tool.digest_failed",
                format!("Failed to canonicalize {role}: {error}"),
                "Use only canonical serializable tool data.",
            )
        })
}

fn candidate_entry_error(error: crate::ProjectCandidateError) -> AiToolKernelError {
    kernel_error(error.code, error.message, error.next_action)
}

fn goal_mutation_error(error: crate::GoalMutationError) -> AiToolKernelError {
    kernel_error(error.code, error.message, error.next_action)
}

fn write_scope_error(error: crate::ProjectWriteError) -> AiToolKernelError {
    kernel_error(
        error.code,
        format!("Tool Kernel project metadata operation failed: {error}"),
        "Repair the project write scope before executing another tool.",
    )
}

fn kernel_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> AiToolKernelError {
    AiToolKernelError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

fn goal_grant_kernel_error(error: crate::AiGoalGrantError) -> AiToolKernelError {
    kernel_error(
        error.code,
        error.message,
        "Discard the invalid goal grant content and request a fresh Native Editor approval.",
    )
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod project_create_contract_tests {
    use super::*;

    #[test]
    fn project_create_contract_is_exactly_two_caller_owned_fields() {
        let registry = AiToolContractRegistry::new();
        let descriptor = registry
            .descriptor("project.create")
            .expect("project.create must be registered");
        let properties = descriptor.input_schema["properties"]
            .as_object()
            .expect("properties");

        assert_eq!(
            properties.keys().cloned().collect::<BTreeSet<_>>(),
            [
                "projectName".to_string(),
                "requestedProjectRoot".to_string()
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            descriptor.input_schema["additionalProperties"],
            Value::Bool(false)
        );
        assert!(registry
            .validate_direct_input(
                "project.create",
                &json!({
                    "requestedProjectRoot": "G:/Projects/NewGame",
                    "projectName": "NewGame",
                    "projectDigest": "caller-owned"
                })
            )
            .is_err());
    }
}
