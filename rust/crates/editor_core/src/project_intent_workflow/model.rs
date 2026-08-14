use crate::{
    ControlledSourcePatchValidationRequest, ProjectCandidateApplyReceipt, ProjectCandidatePayload,
    ProjectCandidateSourceKind,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const INTENT_EVENT_SCHEMA_VERSION: &str = "project-intent-event.v1";
pub const WORK_ITEM_SCHEMA_VERSION: &str = "project-work-item.v1";
pub const PROJECT_INTENT_SNAPSHOT_SCHEMA_VERSION: &str = "project-intent-snapshot.v1";
pub const PROJECT_INTENT_JOURNAL_SCHEMA_VERSION: &str = "project-intent-journal.v1";
pub const PROJECT_DIAGNOSIS_SCHEMA_VERSION: &str = "project-diagnosis-session.v1";
pub const CHANGE_SET_PROPOSAL_SCHEMA_VERSION: &str = "project-change-set-proposal.v1";
pub const CHANGE_SET_APPROVAL_SCHEMA_VERSION: &str = "project-change-set-approval.v1";
pub const PROJECT_PRODUCTION_RUN_SCHEMA_VERSION: &str = "project-production-run.v1";
pub const PROJECT_INTENT_PROJECT_BINDING_SCHEMA_VERSION: &str = "project-intent-project-binding.v1";
pub const SANITIZED_PROJECT_INTENT_CONTEXT_SCHEMA_VERSION: &str =
    "sanitized-project-intent-context.v1";
pub const INTENT_NORMALIZATION_PROPOSAL_SCHEMA_VERSION: &str = "intent-normalization-proposal.v1";
pub const IMPORTED_CHANGE_PLAN_SOURCE_SCHEMA_VERSION: &str = "imported-change-plan-source.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectIntentStorageKind {
    InMemory,
    PreProjectDraft,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentSourceKind {
    UserMessage,
    Screenshot,
    Log,
    TestResult,
    EditorObservation,
    ImportedContext,
    SystemEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPrivacyClass {
    LocalOnly,
    Sanitized,
    Shareable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentCaptureInput {
    pub command_id: String,
    pub project_identity: Option<String>,
    pub occurred_at: Option<String>,
    pub source_kind: IntentSourceKind,
    pub source_identity: String,
    pub content_ref: Option<String>,
    pub sanitized_summary: String,
    #[serde(default)]
    pub attachment_refs: Vec<String>,
    #[serde(default)]
    pub related_event_ids: Vec<String>,
    pub privacy_class: IntentPrivacyClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentEvent {
    pub schema_version: String,
    pub event_id: String,
    pub project_identity: Option<String>,
    pub occurred_at: String,
    pub source_kind: IntentSourceKind,
    pub source_identity: String,
    pub content_ref: Option<String>,
    pub sanitized_summary: String,
    pub attachment_refs: Vec<String>,
    pub related_event_ids: Vec<String>,
    pub privacy_class: IntentPrivacyClass,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentCaptureReceipt {
    pub schema_version: String,
    pub command_id: String,
    pub event_id: String,
    pub journal_revision: u64,
    pub journal_digest: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemKind {
    Idea,
    Requirement,
    Change,
    Bug,
    Question,
    Feedback,
    Experiment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    Captured,
    Triaging,
    NeedsClarification,
    NeedsEvidence,
    Ready,
    Parked,
    Blocked,
    Proposed,
    InProgress,
    Verifying,
    Done,
    Cancelled,
    Merged,
    Split,
}

impl WorkItemStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Cancelled | Self::Merged | Self::Split
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemRelationshipKind {
    DependsOn,
    Blocks,
    RelatedTo,
    Duplicates,
    Supersedes,
    CausedBy,
    EvidenceFor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItemRelationship {
    pub kind: WorkItemRelationshipKind,
    pub target_work_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItemDraft {
    pub kind: WorkItemKind,
    pub title: String,
    pub user_visible_outcome: String,
    pub source_event_ids: Vec<String>,
    pub status: WorkItemStatus,
    pub priority: WorkItemPriority,
    #[serde(default)]
    pub scope_hints: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub relationship_refs: Vec<WorkItemRelationship>,
    pub latest_understanding: String,
    #[serde(default)]
    pub explicitly_deferred: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItem {
    pub schema_version: String,
    pub work_item_id: String,
    pub kind: WorkItemKind,
    pub title: String,
    pub user_visible_outcome: String,
    pub source_event_ids: Vec<String>,
    pub status: WorkItemStatus,
    pub priority: WorkItemPriority,
    pub scope_hints: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub open_questions: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub relationship_refs: Vec<WorkItemRelationship>,
    pub latest_understanding: String,
    pub explicitly_deferred: Vec<String>,
    pub prior_work_item_ids: Vec<String>,
    pub revision: u64,
    pub work_item_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization_source_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SanitizedIntentEventContext {
    pub event_id: String,
    pub source_kind: IntentSourceKind,
    pub sanitized_summary: String,
    pub related_event_ids: Vec<String>,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SanitizedWorkItemContext {
    pub work_item_id: String,
    pub kind: WorkItemKind,
    pub title: String,
    pub status: WorkItemStatus,
    pub source_event_ids: Vec<String>,
    pub latest_understanding: String,
    pub open_questions: Vec<String>,
    pub revision: u64,
    pub work_item_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SanitizedProjectIntentContext {
    pub schema_version: String,
    pub project_identity: Option<String>,
    pub journal_revision: u64,
    pub journal_digest: String,
    pub intent_events: Vec<SanitizedIntentEventContext>,
    pub work_items: Vec<SanitizedWorkItemContext>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentNormalizationProposal {
    pub schema_version: String,
    pub source_label: String,
    pub base_journal_digest: String,
    pub work_items: Vec<WorkItemDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItemRevisionBinding {
    pub work_item_id: String,
    pub revision: u64,
    pub work_item_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisState {
    NeedsEvidence,
    Reproducing,
    Investigating,
    CauseConfirmed,
    Inconclusive,
    FixScopeReady,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosisHypothesis {
    pub hypothesis_id: String,
    pub summary: String,
    pub rejected_by_evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiagnosisSession {
    pub schema_version: String,
    pub diagnosis_id: String,
    pub work_item_id: String,
    pub base_project_digest: Option<String>,
    pub state: DiagnosisState,
    pub reproduction_attempts: Vec<String>,
    pub observations: Vec<String>,
    pub hypotheses: Vec<DiagnosisHypothesis>,
    pub confirmed_cause: Option<String>,
    pub evidence_refs: Vec<String>,
    pub proposed_fix_scope: Vec<String>,
    pub diagnosis_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCapability {
    ReadProject,
    RunExistingTest,
    RunPreview,
    Reproduce,
    WriteIsolatedEvidence,
    ModifyProject,
    AddInstrumentation,
    ChangeDependency,
    ExternalNetwork,
}

impl DiagnosticCapability {
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::ReadProject
                | Self::RunExistingTest
                | Self::RunPreview
                | Self::Reproduce
                | Self::WriteIsolatedEvidence
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosisUpdate {
    pub diagnosis_id: String,
    pub state: DiagnosisState,
    #[serde(default)]
    pub reproduction_attempts: Vec<String>,
    #[serde(default)]
    pub observations: Vec<String>,
    #[serde(default)]
    pub hypotheses: Vec<DiagnosisHypothesis>,
    pub confirmed_cause: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub proposed_fix_scope: Vec<String>,
    #[serde(default)]
    pub requested_capabilities: Vec<DiagnosticCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateSpec {
    pub project_root: String,
    pub project_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetTargetKind {
    NewProject,
    ExistingProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePayloadKind {
    AssetImport,
    ProjectPatch,
    ControlledSourcePatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateValidationProfile {
    pub controlled_source_patch: Option<ControlledSourcePatchValidationRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_source_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidatePlanStep {
    pub step_id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub payload_kind: CandidatePayloadKind,
    pub payload_source_digest: String,
    pub source_kind: ProjectCandidateSourceKind,
    pub source_label: String,
    pub payload: ProjectCandidatePayload,
    pub validation_profile: CandidateValidationProfile,
    #[serde(default)]
    pub expected_changed_domains: Vec<String>,
    pub user_visible_outcome: String,
    pub failure_policy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangePreparationRequest {
    pub command_id: String,
    pub target_kind: ChangeSetTargetKind,
    pub target_project_identity: Option<String>,
    pub project_create_spec: Option<ProjectCreateSpec>,
    pub expected_base_project_digest: Option<String>,
    pub selected_work_item_ids: Vec<String>,
    #[serde(default)]
    pub explicit_exclusions: Vec<String>,
    pub candidate_plan_steps: Vec<CandidatePlanStep>,
    #[serde(default)]
    pub acceptance_checks: Vec<String>,
    #[serde(default)]
    pub estimated_external_waits: Vec<String>,
    #[serde(default)]
    pub external_costs: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub required_decisions: Vec<String>,
    pub repair_policy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedChangePlanSource {
    pub schema_version: String,
    pub source_label: String,
    pub base_journal_digest: String,
    pub target_kind: ChangeSetTargetKind,
    pub target_project_identity: Option<String>,
    pub project_create_spec: Option<ProjectCreateSpec>,
    pub expected_base_project_digest: Option<String>,
    pub selected_work_item_ids: Vec<String>,
    #[serde(default)]
    pub explicit_exclusions: Vec<String>,
    pub candidate_plan_steps: Vec<CandidatePlanStep>,
    #[serde(default)]
    pub acceptance_checks: Vec<String>,
    #[serde(default)]
    pub estimated_external_waits: Vec<String>,
    #[serde(default)]
    pub external_costs: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub required_decisions: Vec<String>,
    pub repair_policy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeSetProposal {
    pub schema_version: String,
    pub proposal_id: String,
    pub target_kind: ChangeSetTargetKind,
    pub target_project_identity: Option<String>,
    pub project_create_spec: Option<ProjectCreateSpec>,
    pub expected_base_project_digest: Option<String>,
    pub selected_work_item_revisions: Vec<WorkItemRevisionBinding>,
    pub user_visible_outcomes: Vec<String>,
    pub explicit_exclusions: Vec<String>,
    pub candidate_plan_steps: Vec<CandidatePlanStep>,
    pub acceptance_checks: Vec<String>,
    pub estimated_external_waits: Vec<String>,
    pub external_costs: Vec<String>,
    pub risks: Vec<String>,
    pub required_decisions: Vec<String>,
    pub repair_policy: String,
    pub proposal_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangePreparationBlocker {
    pub code: String,
    pub work_item_id: Option<String>,
    pub message: String,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "result", rename_all = "snake_case")]
pub enum ChangePreparationResult {
    Ready(ChangeSetProposal),
    Blocked(Vec<ChangePreparationBlocker>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeSetApprovalInput {
    pub command_id: String,
    pub approval_id: String,
    pub approved_by: String,
    pub proposal_digest: String,
    pub target_identity: String,
    pub expected_base_project_digest: Option<String>,
    #[serde(default)]
    pub approved_risk_classes: Vec<String>,
    #[serde(default)]
    pub approved_external_costs: Vec<String>,
    pub approved_repair_policy: String,
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeSetApproval {
    pub schema_version: String,
    pub approval_id: String,
    pub approved_by: String,
    pub proposal_digest: String,
    pub selected_work_item_digests: Vec<String>,
    pub target_identity: String,
    pub expected_base_project_digest: Option<String>,
    pub approved_risk_classes: Vec<String>,
    pub approved_external_costs: Vec<String>,
    pub approved_repair_policy: String,
    pub approved_at: String,
    pub approval_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProductionRunKind {
    FromBlank,
    ScopedChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProductionRunState {
    Approved,
    CreatingProject,
    Executing,
    Previewing,
    Completed,
    PausedForDecision,
    Stale,
    Failed,
    Cancelled,
}

impl ProjectProductionRunState {
    pub fn holds_mutation_lane(self) -> bool {
        matches!(
            self,
            Self::Approved | Self::CreatingProject | Self::Executing | Self::PausedForDecision
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionStepState {
    Pending,
    Validating,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionStepSnapshot {
    pub step_id: String,
    pub state: ProductionStepState,
    pub candidate_id: Option<String>,
    pub candidate_digest: Option<String>,
    pub validation_digest: Option<String>,
    pub apply_receipt: Option<ProjectCandidateApplyReceipt>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectProductionRun {
    pub schema_version: String,
    pub run_id: String,
    pub run_kind: ProjectProductionRunKind,
    pub proposal_id: String,
    pub change_set_approval_digest: String,
    pub target_project_identity: String,
    pub base_project_digest: Option<String>,
    pub current_project_digest: Option<String>,
    pub state: ProjectProductionRunState,
    pub active_step_id: Option<String>,
    pub step_snapshots: Vec<ProductionStepSnapshot>,
    pub linked_work_item_revisions: Vec<WorkItemRevisionBinding>,
    pub decision_requests: Vec<String>,
    pub recovery_options: Vec<String>,
    pub preview_evidence: Option<String>,
    pub diagnostics: Vec<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntentProjectBinding {
    pub schema_version: String,
    pub project_id: String,
    pub project_root: String,
    pub initial_project_digest: String,
    pub creation_receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "recordKind", content = "record", rename_all = "snake_case")]
pub enum ProjectIntentJournalRecord {
    IntentCaptured(IntentEvent),
    WorkItemChanged(WorkItem),
    WorkItemsChanged(Vec<WorkItem>),
    DiagnosisChanged(ProjectDiagnosisSession),
    ChangeSetPrepared {
        proposal: ChangeSetProposal,
        work_items: Vec<WorkItem>,
    },
    RunAuthorized {
        approval: ChangeSetApproval,
        run: ProjectProductionRun,
        work_items: Vec<WorkItem>,
    },
    RunChanged {
        run: ProjectProductionRun,
        work_items: Vec<WorkItem>,
    },
    ProjectAttached(ProjectIntentProjectBinding),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntentJournalEntry {
    pub revision: u64,
    pub command_id: String,
    pub occurred_at: String,
    pub record: ProjectIntentJournalRecord,
    pub entry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntentJournalDocument {
    pub schema_version: String,
    pub project_binding: Option<ProjectIntentProjectBinding>,
    pub revision: u64,
    pub entries: Vec<ProjectIntentJournalEntry>,
    pub journal_digest: String,
}

impl Default for ProjectIntentJournalDocument {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_INTENT_JOURNAL_SCHEMA_VERSION.to_string(),
            project_binding: None,
            revision: 0,
            entries: Vec::new(),
            journal_digest: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItemSummary {
    pub work_item_id: String,
    pub kind: WorkItemKind,
    pub title: String,
    pub status: WorkItemStatus,
    pub ready: bool,
    pub revision: u64,
    pub work_item_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosisSummary {
    pub diagnosis_id: String,
    pub work_item_id: String,
    pub state: DiagnosisState,
    pub diagnosis_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntentSnapshot {
    pub schema_version: String,
    pub checkpoint_id: String,
    pub journal_revision: u64,
    pub journal_digest: String,
    pub project_binding: Option<ProjectIntentProjectBinding>,
    pub intent_events: Vec<IntentEvent>,
    pub work_items: Vec<WorkItem>,
    pub work_item_summaries: Vec<WorkItemSummary>,
    pub active_diagnoses: Vec<ProjectDiagnosisSession>,
    pub active_diagnosis_summaries: Vec<DiagnosisSummary>,
    pub active_proposal: Option<ChangeSetProposal>,
    pub active_approval: Option<ChangeSetApproval>,
    pub active_run: Option<ProjectProductionRun>,
    pub pending_normalization_event_ids: Vec<String>,
    pub processed_commands: BTreeMap<String, u64>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectGoalSnapshot {
    pub schema_version: String,
    pub snapshot_id: String,
    pub project_identity: Option<String>,
    pub included_work_item_revisions: Vec<WorkItemRevisionBinding>,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub explicitly_deferred: Vec<String>,
    pub snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "queryKind", content = "query", rename_all = "snake_case")]
pub enum ProjectIntentQuery {
    All,
    WorkItems { work_item_ids: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "commandKind", content = "command", rename_all = "snake_case")]
pub enum ProjectIntentWorkflowCommand {
    CreateWorkItem {
        command_id: String,
        draft: WorkItemDraft,
    },
    ReviseWorkItem {
        command_id: String,
        work_item_id: String,
        draft: WorkItemDraft,
    },
    ParkWorkItem {
        command_id: String,
        work_item_id: String,
    },
    ResumeWorkItem {
        command_id: String,
        work_item_id: String,
    },
    CancelWorkItem {
        command_id: String,
        work_item_id: String,
    },
    ReopenWorkItem {
        command_id: String,
        work_item_id: String,
        evidence_refs: Vec<String>,
    },
    MergeWorkItems {
        command_id: String,
        source_work_item_ids: Vec<String>,
        merged: WorkItemDraft,
    },
    SplitWorkItem {
        command_id: String,
        source_work_item_id: String,
        parts: Vec<WorkItemDraft>,
    },
    StartDiagnosis {
        command_id: String,
        work_item_id: String,
        base_project_digest: Option<String>,
    },
    UpdateDiagnosis {
        command_id: String,
        update: DiagnosisUpdate,
    },
    AdvanceRun {
        command_id: String,
        run_id: String,
    },
    CancelRun {
        command_id: String,
        run_id: String,
    },
    RecoverRun {
        command_id: String,
        run_id: String,
    },
}

impl ProjectIntentWorkflowCommand {
    pub fn command_id(&self) -> &str {
        match self {
            Self::CreateWorkItem { command_id, .. }
            | Self::ReviseWorkItem { command_id, .. }
            | Self::ParkWorkItem { command_id, .. }
            | Self::ResumeWorkItem { command_id, .. }
            | Self::CancelWorkItem { command_id, .. }
            | Self::ReopenWorkItem { command_id, .. }
            | Self::MergeWorkItems { command_id, .. }
            | Self::SplitWorkItem { command_id, .. }
            | Self::StartDiagnosis { command_id, .. }
            | Self::UpdateDiagnosis { command_id, .. }
            | Self::AdvanceRun { command_id, .. }
            | Self::CancelRun { command_id, .. }
            | Self::RecoverRun { command_id, .. } => command_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntentWorkflowError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl ProjectIntentWorkflowError {
    pub(crate) fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            next_action: next_action.into(),
        }
    }
}

impl std::fmt::Display for ProjectIntentWorkflowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectIntentWorkflowError {}
