use serde::{Deserialize, Serialize};

pub const RULE_AUTHORING_REPORT_SCHEMA_VERSION: &str = "rule-authoring-report.v1";
pub const RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION: &str = "rule-card-authoring-report.v1";
pub const RULE_GRAPH_PREVIEW_SCHEMA_VERSION: &str = "rule-graph-preview.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAuthoringStatus {
    Missing,
    Ready,
    Dirty,
    Valid,
    Invalid,
    Built,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAuthoringDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringDiagnostic {
    pub severity: RuleAuthoringDiagnosticSeverity,
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub human_explanation: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringReport {
    pub schema_version: String,
    pub status: RuleAuthoringStatus,
    pub asset_id: Option<String>,
    pub rule_id: Option<String>,
    pub ir_hash: Option<String>,
    pub human_summary: String,
    pub diagnostics: Vec<RuleAuthoringDiagnostic>,
    pub changed_paths: Vec<String>,
    pub next_actions: Vec<String>,
    pub generated_rust_source: RuleAuthoringStageEvidence,
    pub static_registry_source: RuleAuthoringStageEvidence,
    pub artifact_lifecycle: RuleAuthoringStageEvidence,
    pub runtime_package_manifest: RuleAuthoringStageEvidence,
    pub cargo_build: RuleAuthoringStageEvidence,
}

impl RuleAuthoringReport {
    pub fn missing() -> Self {
        Self {
            schema_version: RULE_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status: RuleAuthoringStatus::Missing,
            asset_id: None,
            rule_id: None,
            ir_hash: None,
            human_summary: "No rule asset is selected.".to_string(),
            diagnostics: Vec::new(),
            changed_paths: Vec::new(),
            next_actions: vec!["create_rule_asset".to_string()],
            generated_rust_source: RuleAuthoringStageEvidence::skipped(
                "not_requested",
                "Validate or build a selected rule asset.",
            ),
            static_registry_source: RuleAuthoringStageEvidence::skipped(
                "not_requested",
                "Build a selected rule asset.",
            ),
            artifact_lifecycle: RuleAuthoringStageEvidence::skipped(
                "not_requested",
                "Build a selected rule asset.",
            ),
            runtime_package_manifest: RuleAuthoringStageEvidence::skipped(
                "not_requested",
                "Export a RuntimePackage after rule build.",
            ),
            cargo_build: RuleAuthoringStageEvidence::skipped(
                "skipped_by_v1",
                "Rule Authoring v1 does not run cargo build for every editor validation.",
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringStageEvidence {
    pub status: RuleAuthoringStageStatus,
    pub path: Option<String>,
    pub artifact_id: Option<String>,
    pub summary: String,
    pub skip_reason: Option<String>,
    pub next_action: Option<String>,
}

impl RuleAuthoringStageEvidence {
    pub fn skipped(skip_reason: impl Into<String>, next_action: impl Into<String>) -> Self {
        Self {
            status: RuleAuthoringStageStatus::SkippedByV1,
            path: None,
            artifact_id: None,
            summary: "skipped".to_string(),
            skip_reason: Some(skip_reason.into()),
            next_action: Some(next_action.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAuthoringStageStatus {
    NotRequested,
    Produced,
    Validated,
    Ready,
    Blocked,
    SkippedByV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringDocument {
    pub asset_path: Option<String>,
    pub asset_id: Option<String>,
    pub rule_id: Option<String>,
    pub display_name: Option<String>,
    pub dirty: bool,
    pub selected_statement_path: Option<String>,
    pub selected_operation_path: Option<String>,
    pub human_summary: String,
    pub report: RuleAuthoringReport,
}

impl RuleAuthoringDocument {
    pub fn empty() -> Self {
        Self {
            asset_path: None,
            asset_id: None,
            rule_id: None,
            display_name: None,
            dirty: false,
            selected_statement_path: None,
            selected_operation_path: None,
            human_summary: "No rule asset is selected.".to_string(),
            report: RuleAuthoringReport::missing(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringPatch {
    pub patch_id: String,
    pub asset_id: Option<String>,
    pub expected_ir_hash: Option<String>,
    pub operations: Vec<RuleAuthoringPatchOperation>,
    pub source: RuleAuthoringPatchSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAuthoringPatchSource {
    User,
    Ai,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum RuleAuthoringPatchOperation {
    CreateRuleAsset { path: String, rule_id: String },
    SetRuleTrigger { trigger_kind: String, value: String },
    AddStatement { statement_kind: String },
    UpdateStatement { path: String },
    RemoveStatement { path: String },
    AddOperation { operation_kind: String },
    UpdateOperation { path: String },
    RemoveOperation { path: String },
    ValidateRuleAsset,
    CompileRuleAsset,
    SaveRuleAsset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringCommand {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
    pub reason_disabled: Option<String>,
}

impl RuleAuthoringCommand {
    pub fn new(
        command_id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        reason_disabled: Option<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            label: label.into(),
            enabled,
            reason_disabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleCardKind {
    Trigger,
    Statement,
    Operation,
    Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleCardFieldValueKind {
    String,
    Number,
    Bool,
    Object,
    Array,
    Enum,
    AssetRef,
    RuntimeValue,
    Json,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleCardValidationState {
    Unknown,
    Valid,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardDiagnosticRef {
    pub code: String,
    pub source_path: Option<String>,
    pub severity: RuleAuthoringDiagnosticSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardFieldModel {
    pub field_id: String,
    pub label: String,
    pub field_path: String,
    pub value_kind: RuleCardFieldValueKind,
    pub value_preview: String,
    pub editable: bool,
    pub enum_options: Vec<String>,
    pub asset_ref_options: Vec<String>,
    pub validation_state: RuleCardValidationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardModel {
    pub card_id: String,
    pub kind: RuleCardKind,
    pub asset_path: Option<String>,
    pub rule_id: Option<String>,
    pub source_path: String,
    pub title: String,
    pub summary: String,
    pub human_explanation: String,
    pub fields: Vec<RuleCardFieldModel>,
    pub allowed_commands: Vec<RuleAuthoringCommand>,
    pub diagnostics: Vec<RuleCardDiagnosticRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleGraphPreviewNodeKind {
    Trigger,
    Statement,
    Operation,
    Diagnostic,
    Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleGraphPreviewNodeStatus {
    Normal,
    Warning,
    Error,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleGraphPreviewEdgeKind {
    ExecutionOrder,
    DiagnosticTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGraphPreviewNode {
    pub node_id: String,
    pub card_id: Option<String>,
    pub source_path: String,
    pub kind: RuleGraphPreviewNodeKind,
    pub label: String,
    pub status: RuleGraphPreviewNodeStatus,
    pub diagnostic_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGraphPreviewEdge {
    pub edge_id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub kind: RuleGraphPreviewEdgeKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGraphPreviewGroup {
    pub group_id: String,
    pub label: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardSourceMapping {
    pub source_path: String,
    pub card_id: Option<String>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGraphPreviewModel {
    pub schema_version: String,
    pub asset_path: Option<String>,
    pub rule_id: Option<String>,
    pub ir_hash: Option<String>,
    pub nodes: Vec<RuleGraphPreviewNode>,
    pub edges: Vec<RuleGraphPreviewEdge>,
    pub groups: Vec<RuleGraphPreviewGroup>,
    pub selected_node_id: Option<String>,
    pub source_mappings: Vec<RuleCardSourceMapping>,
    pub read_only: bool,
}

impl RuleGraphPreviewModel {
    pub fn empty() -> Self {
        Self {
            schema_version: RULE_GRAPH_PREVIEW_SCHEMA_VERSION.to_string(),
            asset_path: None,
            rule_id: None,
            ir_hash: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
            selected_node_id: None,
            source_mappings: Vec::new(),
            read_only: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardAuthoringReport {
    pub schema_version: String,
    pub status: RuleAuthoringStatus,
    pub asset_path: Option<String>,
    pub rule_id: Option<String>,
    pub ir_hash: Option<String>,
    pub card_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub editable_card_count: usize,
    pub read_only_graph: bool,
    pub changed_paths: Vec<String>,
    pub diagnostics: Vec<RuleAuthoringDiagnostic>,
    pub next_actions: Vec<String>,
    pub source_mappings: Vec<RuleCardSourceMapping>,
}

impl RuleCardAuthoringReport {
    pub fn missing() -> Self {
        Self {
            schema_version: RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status: RuleAuthoringStatus::Missing,
            asset_path: None,
            rule_id: None,
            ir_hash: None,
            card_count: 0,
            graph_node_count: 0,
            graph_edge_count: 0,
            editable_card_count: 0,
            read_only_graph: true,
            changed_paths: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: vec!["create_rule_asset".to_string()],
            source_mappings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardAuthoringModel {
    pub project_root: Option<String>,
    pub selected_path: Option<String>,
    pub rule_count: usize,
    pub document: RuleAuthoringDocument,
    pub selected_card_id: Option<String>,
    pub cards: Vec<RuleCardModel>,
    pub graph_preview: RuleGraphPreviewModel,
    pub commands: Vec<RuleAuthoringCommand>,
    pub report_summary: RuleCardAuthoringReport,
}

impl RuleCardAuthoringModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            selected_path: None,
            rule_count: 0,
            document: RuleAuthoringDocument::empty(),
            selected_card_id: None,
            cards: Vec::new(),
            graph_preview: RuleGraphPreviewModel::empty(),
            commands: vec![
                RuleAuthoringCommand::new(
                    "select_rule_card",
                    "Select Card",
                    false,
                    Some("Select a rule asset first.".to_string()),
                ),
                RuleAuthoringCommand::new(
                    "set_rule_card_field",
                    "Edit Card Field",
                    false,
                    Some("Select a rule asset first.".to_string()),
                ),
            ],
            report_summary: RuleCardAuthoringReport::missing(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAuthoringModel {
    pub project_root: Option<String>,
    pub selected_path: Option<String>,
    pub rule_count: usize,
    pub document: RuleAuthoringDocument,
    pub card_authoring: RuleCardAuthoringModel,
    pub commands: Vec<RuleAuthoringCommand>,
    pub empty_message: String,
}

impl RuleAuthoringModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            selected_path: None,
            rule_count: 0,
            document: RuleAuthoringDocument::empty(),
            card_authoring: RuleCardAuthoringModel::empty(),
            commands: vec![RuleAuthoringCommand::new(
                "create_rule_asset",
                "Create Rule",
                false,
                Some("Open a project first.".to_string()),
            )],
            empty_message: "Open a project to author rules.".to_string(),
        }
    }
}
