use editor_core::{command_for_test, scan_rule_asset_paths, CommandStatus, RuleAuthoringService};
use editor_ui_model::{RuleCardSourceMapping, UiCommandPayload};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-rule-card-authoring-productization-report.v1";
pub const COMPLEX_SHOOTER_RULE_CARD_AUTHORING_SCENARIO_ID: &str =
    "complex_shooter_rule_card_authoring_productization";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexShooterRuleCardAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardAuthoringRuleEvidence {
    pub rule_path: String,
    pub rule_id: Option<String>,
    pub card_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub read_only_graph: bool,
    pub editable_card_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCardAuthoringEditEvidence {
    pub path: String,
    pub command_status: String,
    pub changed_paths: Vec<String>,
    pub validate_status: String,
    pub build_status: String,
    pub graph_refreshed: bool,
    pub graph_node_count_after_edit: usize,
    pub operation_card_present_after_edit: bool,
    pub source_mappings: Vec<RuleCardSourceMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterRuleCardAuthoringReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterRuleCardAuthoringStatus,
    pub project_root: String,
    pub output_root: String,
    pub rule_asset_count: usize,
    pub covered_rule_paths: Vec<String>,
    pub rule_evidence: Vec<RuleCardAuthoringRuleEvidence>,
    pub edit_evidence: Option<RuleCardAuthoringEditEvidence>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterRuleCardAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterRuleCardAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_rule_card_authoring_report(
    request: ComplexShooterRuleCardAuthoringRequest,
) -> ComplexShooterRuleCardAuthoringReport {
    let mut diagnostics = Vec::new();
    let mut next_actions = Vec::new();
    let mut artifacts = Vec::new();
    let rule_paths = scan_rule_asset_paths(&request.project_root);
    let target_paths = vec![
        "Rules/fire_bullet.rule.json".to_string(),
        "Rules/linear_motion.rule.json".to_string(),
        "Rules/lifetime_cleanup.rule.json".to_string(),
    ];
    let covered_rule_paths = target_paths
        .iter()
        .filter(|path| rule_paths.contains(path))
        .cloned()
        .collect::<Vec<_>>();
    for path in &target_paths {
        if !rule_paths.contains(path) {
            diagnostics.push(format!("target_rule_missing:{path}"));
        }
    }

    let rule_evidence = covered_rule_paths
        .iter()
        .map(|path| {
            let model =
                RuleAuthoringService::build_model(Some(&request.project_root), Some(path.clone()));
            RuleCardAuthoringRuleEvidence {
                rule_path: path.clone(),
                rule_id: model.document.rule_id.clone(),
                card_count: model.card_authoring.report_summary.card_count,
                graph_node_count: model.card_authoring.report_summary.graph_node_count,
                graph_edge_count: model.card_authoring.report_summary.graph_edge_count,
                read_only_graph: model.card_authoring.report_summary.read_only_graph,
                editable_card_count: model.card_authoring.report_summary.editable_card_count,
            }
        })
        .collect::<Vec<_>>();

    let edit_evidence = run_card_edit_evidence(
        &request.project_root,
        "Rules/fire_bullet.rule.json",
        &mut diagnostics,
    );
    if edit_evidence.is_none() {
        next_actions.push("fix_rule_card_authoring_edit_command".to_string());
    }
    if rule_evidence.len() < target_paths.len() {
        next_actions.push("restore_complex_shooter_rule_authoring_assets".to_string());
    }
    if rule_evidence
        .iter()
        .any(|evidence| !evidence.read_only_graph || evidence.card_count == 0)
    {
        diagnostics.push("rule_card_or_graph_preview_missing".to_string());
        next_actions.push("derive_rule_card_authoring_model_from_rule_asset".to_string());
    }

    let status = if diagnostics.is_empty() {
        ComplexShooterRuleCardAuthoringStatus::Passed
    } else if edit_evidence.is_some() && !covered_rule_paths.is_empty() {
        ComplexShooterRuleCardAuthoringStatus::Partial
    } else {
        ComplexShooterRuleCardAuthoringStatus::Failed
    };

    let report = ComplexShooterRuleCardAuthoringReport {
        schema_version: COMPLEX_SHOOTER_RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_RULE_CARD_AUTHORING_SCENARIO_ID.to_string(),
        status,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        rule_asset_count: rule_paths.len(),
        covered_rule_paths,
        rule_evidence,
        edit_evidence,
        artifacts: Vec::new(),
        diagnostics,
        next_actions,
    };

    let report_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-rule-card-authoring-productization-report.json");
    if write_report(&report_path, &report).is_ok() {
        artifacts.push(report_path.display().to_string());
    }

    ComplexShooterRuleCardAuthoringReport {
        artifacts,
        ..report
    }
}

fn run_card_edit_evidence(
    project_root: &Path,
    path: &str,
    diagnostics: &mut Vec<String>,
) -> Option<RuleCardAuthoringEditEvidence> {
    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));
    if open.status != CommandStatus::Committed {
        diagnostics.push("open_project_failed_for_rule_card_authoring".to_string());
        return None;
    }
    let before_asset = RuleAuthoringService::load(project_root, path).ok()?;
    let before_hash = before_asset.ir_hash();
    let edit = session.execute_command(command_for_test(UiCommandPayload::AddRuleCard {
        path: path.to_string(),
        card_kind: "operation".to_string(),
        value: serde_json::json!({
            "op": "emitEvent",
            "event_type": "project.rule_card_authoring_smoke"
        }),
        expected_ir_hash: Some(before_hash),
    }));
    if edit.status != CommandStatus::Committed {
        diagnostics.push(format!("add_rule_card_failed:{:?}", edit.status));
        return None;
    }
    let refresh = session.execute_command(command_for_test(
        UiCommandPayload::RefreshRuleGraphPreview {
            path: path.to_string(),
        },
    ));
    let after_model = session.build_rule_authoring_model();
    let validate = RuleAuthoringService::load(project_root, path)
        .map(|asset| RuleAuthoringService::validate(&asset))
        .ok()?;
    let build = RuleAuthoringService::build(project_root, path).ok()?;
    let changed_paths = edit
        .state_changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();

    Some(RuleCardAuthoringEditEvidence {
        path: path.to_string(),
        command_status: format!("{:?}", edit.status),
        changed_paths,
        validate_status: format!("{:?}", validate.status),
        build_status: format!("{:?}", build.status),
        graph_refreshed: refresh.status == CommandStatus::Committed,
        graph_node_count_after_edit: after_model.card_authoring.graph_preview.nodes.len(),
        operation_card_present_after_edit: after_model
            .card_authoring
            .cards
            .iter()
            .any(|card| card.card_id == "card:operation:0"),
        source_mappings: after_model
            .card_authoring
            .graph_preview
            .source_mappings
            .clone(),
    })
}

fn write_report(path: &Path, report: &ComplexShooterRuleCardAuthoringReport) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create report directory: {error}"))?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("Failed to serialize rule card authoring report: {error}"))?;
    fs::write(path, json)
        .map_err(|error| format!("Failed to write rule card authoring report: {error}"))
}
