use serde::{Deserialize, Serialize};

use super::{EditorDiagnostic, UiCommandPayload};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiPanelModel {
    pub prompt_placeholder: String,
    pub prompt_draft: String,
    pub messages: Vec<AiPanelMessage>,
    #[serde(default)]
    pub gateway_access: GatewayAccessInboxModel,
    pub proposed_commands: Vec<AiProposedCommand>,
    pub allowed_command_ids: Vec<String>,
    pub busy: bool,
    pub stage: AiPanelStage,
    pub status_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GatewayAccessInboxModel {
    pub requests: Vec<GatewayAccessRequestModel>,
    pub page_index: usize,
    pub page_count: usize,
    pub total_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayAccessRequestModel {
    pub request_id: String,
    pub operation_short_id: String,
    pub client_session_id: String,
    pub session_short_id: String,
    pub client_kind: String,
    pub client_version: String,
    pub project_identity: String,
    pub connected_age_ms: u64,
    pub expires_in_ms: u64,
    pub state: String,
    pub requested_profile: String,
    pub risk_class: String,
    pub capabilities: Vec<String>,
    pub blocked_capabilities: Vec<String>,
    pub goal_id: String,
    pub user_visible_outcome: String,
    pub completion_policy: String,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_objects: Vec<String>,
    pub max_mutation_count: u32,
    pub time_budget_ms: u64,
    pub external_cost_budget_microunits: u64,
    pub allow_delete: bool,
    pub allow_dependency_change: bool,
    pub allow_network: bool,
    pub approval_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPanelStage {
    Idle,
    Generating,
    Repairing,
    Cancelling,
    Reviewing,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPanelMessage {
    pub message_id: String,
    pub role: AiPanelMessageRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPanelMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiPanelResponse {
    pub explanation: String,
    pub proposed_commands: Vec<AiProposedCommand>,
    pub risk_summary: Option<String>,
    pub requires_confirmation: bool,
    pub diagnostics: Vec<EditorDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiProposedCommand {
    pub proposal_id: String,
    pub label: String,
    pub explanation: String,
    pub command: UiCommandPayload,
    pub project_patch: Option<ProjectPatchEvidence>,
    pub imported_project_patch: Option<ImportedProjectPatchEvidence>,
    pub review_state: AiCommandReviewState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiCommandReviewState {
    Proposed,
    Accepted,
    Rejected,
    Executed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPatchEvidence {
    pub patch_id: String,
    pub patch_title: String,
    pub touched_domains: Vec<String>,
    pub operation_count: usize,
    pub validation_status: bool,
    pub risk_level: String,
    pub repaired_once: bool,
    pub diagnostics: Vec<ProjectPatchDiagnosticEvidence>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPatchDiagnosticEvidence {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub operation_id: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedProjectPatchEvidence {
    pub source_kind: String,
    pub source_label: String,
    pub patch_id: Option<String>,
    pub parse_status: String,
    pub validation_status: Option<bool>,
    pub review_state: String,
}
