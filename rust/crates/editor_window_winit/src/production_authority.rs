use engine_runtime::project_observation::{
    ProjectObservationType, ProjectObservationValue, ProjectRuntimeObservationState,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION: &str =
    "production-editor-authority-scenario.v1";
pub const PRODUCTION_AUTHORITY_REPORT_SCHEMA_VERSION: &str =
    "production-editor-authority-report.v1";
const MAX_SCENARIO_STEPS: usize = 256;
const MAX_STEP_TIMEOUT_MS: u64 = 120_000;
const MAX_OVERALL_TIMEOUT_MS: u64 = 20 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuthorityScenario {
    pub schema_version: String,
    pub scenario_id: String,
    pub evidence_root: PathBuf,
    pub project_root: PathBuf,
    pub recent_project_store_path: PathBuf,
    pub workspace_layout_store_root: PathBuf,
    pub physical_width: u32,
    pub physical_height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_view_target: Option<engine_runtime::game_view_presentation::GameViewTargetSpec>,
    pub per_step_timeout_ms: u64,
    pub overall_timeout_ms: u64,
    pub steps: Vec<ProductionAuthorityStep>,
}

impl ProductionAuthorityScenario {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|error| {
            format!("authority.scenario_read_failed:{}:{error}", path.display())
        })?;
        let scenario: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("authority.scenario_parse_failed:{error}"))?;
        scenario.validate()?;
        Ok(scenario)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION {
            return Err("authority.scenario_schema_unsupported".to_string());
        }
        if self.scenario_id.is_empty()
            || !self
                .scenario_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
        {
            return Err("authority.scenario_id_invalid".to_string());
        }
        if self.steps.is_empty() || self.steps.len() > MAX_SCENARIO_STEPS {
            return Err("authority.scenario_step_count_invalid".to_string());
        }
        if self.physical_width < 640
            || self.physical_height < 360
            || self.physical_width > 7680
            || self.physical_height > 4320
        {
            return Err("authority.scenario_physical_size_invalid".to_string());
        }
        if self.game_view_target.is_some_and(|target| {
            target.extent.width == 0
                || target.extent.height == 0
                || target.extent.width > 8192
                || target.extent.height > 8192
        }) {
            return Err("authority.scenario_game_view_target_invalid".to_string());
        }
        if self.per_step_timeout_ms == 0 || self.per_step_timeout_ms > MAX_STEP_TIMEOUT_MS {
            return Err("authority.scenario_step_timeout_invalid".to_string());
        }
        if self.overall_timeout_ms == 0
            || self.overall_timeout_ms > MAX_OVERALL_TIMEOUT_MS
            || self.overall_timeout_ms < self.per_step_timeout_ms
        {
            return Err("authority.scenario_overall_timeout_invalid".to_string());
        }
        let mut ids = std::collections::BTreeSet::new();
        for step in &self.steps {
            if !ids.insert(step.step_id()) {
                return Err(format!(
                    "authority.scenario_step_id_duplicate:{}",
                    step.step_id()
                ));
            }
            if let ProductionAuthorityStep::WaitFor {
                timeout_ms,
                condition,
                ..
            } = step
            {
                if timeout_ms.is_some_and(|timeout| timeout == 0 || timeout > MAX_STEP_TIMEOUT_MS) {
                    return Err("authority.scenario_wait_timeout_invalid".to_string());
                }
                if let ProductionAuthorityCondition::ProjectValueEquals { path, equals } = condition
                {
                    if path.len() > 128 || !is_stable_observation_path(path) {
                        return Err("authority.project_observation_path_invalid".to_string());
                    }
                    if !matches!(
                        equals,
                        serde_json::Value::Bool(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::String(_)
                    ) {
                        return Err(
                            "authority.project_observation_expected_type_invalid".to_string()
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProductionAuthorityStep {
    ClickEditorWidget {
        step_id: String,
        widget_id: String,
    },
    ClickGameViewAuiNode {
        step_id: String,
        node_id: String,
        #[serde(default)]
        expected_action_id: Option<String>,
    },
    WaitFor {
        step_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        condition: ProductionAuthorityCondition,
    },
    Capture {
        step_id: String,
        checkpoint_id: String,
    },
}

impl ProductionAuthorityStep {
    pub fn step_id(&self) -> &str {
        match self {
            Self::ClickEditorWidget { step_id, .. }
            | Self::ClickGameViewAuiNode { step_id, .. }
            | Self::WaitFor { step_id, .. }
            | Self::Capture { step_id, .. } => step_id,
        }
    }

    pub fn timeout_ms(&self) -> Option<u64> {
        match self {
            Self::WaitFor { timeout_ms, .. } => *timeout_ms,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProductionAuthorityCondition {
    EditorMode {
        mode: String,
    },
    LastCommandId {
        command_id: String,
    },
    ActiveRuntime {
        active: bool,
    },
    RuntimeFrameAtLeast {
        frame_index: u64,
    },
    RuntimeFrameAdvancedSinceStep {
        step_id: String,
        minimum_delta: u64,
    },
    RuntimeActionId {
        action_id: String,
    },
    GameViewAuiNodeActionable {
        node_id: String,
    },
    RuntimeSessionChanged {
        previous_session_id: String,
    },
    RuntimeSessionChangedSinceStep {
        step_id: String,
    },
    ProjectValueEquals {
        path: String,
        equals: serde_json::Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductionAuthorityConditionStatus {
    Passed,
    Pending,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProductionAuthorityProjectValueEvaluation {
    pub status: ProductionAuthorityConditionStatus,
    pub diagnostic_code: Option<String>,
    pub path: String,
    pub declared_type: Option<ProjectObservationType>,
    pub expected: serde_json::Value,
    pub last_actual: Option<serde_json::Value>,
    pub runtime_frame: Option<u64>,
    pub session_id: Option<String>,
    pub contract_id: Option<String>,
}

pub(crate) fn evaluate_project_value_condition(
    path: &str,
    expected: &serde_json::Value,
    state: Option<&ProjectRuntimeObservationState>,
    active_session_id: Option<&str>,
    minimum_runtime_frame_exclusive: Option<u64>,
) -> ProductionAuthorityProjectValueEvaluation {
    let mut evaluation = ProductionAuthorityProjectValueEvaluation {
        status: ProductionAuthorityConditionStatus::Failed,
        diagnostic_code: None,
        path: path.to_string(),
        declared_type: None,
        expected: expected.clone(),
        last_actual: None,
        runtime_frame: state.and_then(ProjectRuntimeObservationState::runtime_frame),
        session_id: state.map(|state| state.session_id().to_string()),
        contract_id: state.map(|state| state.contract_id().to_string()),
    };
    let Some(state) = state else {
        evaluation.diagnostic_code =
            Some("authority.project_observation_contract_unavailable".to_string());
        return evaluation;
    };
    let Some(declared_type) = state.declared_types().get(path).copied() else {
        evaluation.diagnostic_code = Some("authority.project_observation_path_unknown".to_string());
        return evaluation;
    };
    evaluation.declared_type = Some(declared_type);
    if expected_observation_type(expected) != Some(declared_type) {
        evaluation.diagnostic_code =
            Some("authority.project_observation_expected_type_mismatch".to_string());
        return evaluation;
    }
    match state {
        ProjectRuntimeObservationState::NotProducedYet { .. } => {
            evaluation.status = ProductionAuthorityConditionStatus::Pending;
            evaluation
        }
        ProjectRuntimeObservationState::ContractViolated { .. } => {
            evaluation.diagnostic_code =
                Some("authority.project_observation_contract_violated".to_string());
            evaluation
        }
        ProjectRuntimeObservationState::Published { snapshot } => {
            if active_session_id != Some(snapshot.session_id.as_str()) {
                evaluation.diagnostic_code =
                    Some("authority.project_observation_session_changed".to_string());
                return evaluation;
            }
            if minimum_runtime_frame_exclusive
                .is_some_and(|minimum| snapshot.runtime_frame <= minimum)
            {
                evaluation.status = ProductionAuthorityConditionStatus::Pending;
                return evaluation;
            }
            let Some(actual) = snapshot.values.get(path) else {
                evaluation.diagnostic_code =
                    Some("authority.project_observation_contract_violated".to_string());
                return evaluation;
            };
            if actual.value_type() != declared_type || !actual.is_valid_scalar() {
                evaluation.diagnostic_code =
                    Some("authority.project_observation_contract_violated".to_string());
                return evaluation;
            }
            let actual_json = project_observation_value_json(actual);
            evaluation.last_actual = Some(actual_json.clone());
            evaluation.status = if &actual_json == expected {
                ProductionAuthorityConditionStatus::Passed
            } else {
                ProductionAuthorityConditionStatus::Pending
            };
            evaluation
        }
    }
}

fn expected_observation_type(value: &serde_json::Value) -> Option<ProjectObservationType> {
    match value {
        serde_json::Value::Bool(_) => Some(ProjectObservationType::Bool),
        serde_json::Value::Number(number) if number.as_i64().is_some() => {
            Some(ProjectObservationType::Integer)
        }
        serde_json::Value::Number(number) if number.as_f64().is_some() => {
            Some(ProjectObservationType::Number)
        }
        serde_json::Value::String(_) => Some(ProjectObservationType::String),
        _ => None,
    }
}

fn project_observation_value_json(value: &ProjectObservationValue) -> serde_json::Value {
    match value {
        ProjectObservationValue::Bool(value) => serde_json::Value::Bool(*value),
        ProjectObservationValue::Integer(value) => serde_json::json!(*value),
        ProjectObservationValue::Number(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        ProjectObservationValue::String(value) => serde_json::Value::String(value.clone()),
    }
}

fn is_stable_observation_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic())
                && chars.all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuthorityStepReport {
    pub step_id: String,
    pub kind: String,
    pub target: Option<String>,
    pub status: String,
    pub actionable: Option<bool>,
    pub pointer_down_observed: bool,
    pub pointer_up_observed: bool,
    pub before_command_id: Option<String>,
    pub after_command_id: Option<String>,
    pub runtime_action_id: Option<String>,
    pub runtime_frame_index: Option<u64>,
    pub runtime_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_view_coordinates: Option<GameViewAuiCoordinateEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_input_route: Option<crate::ViewportInputRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_declared_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_expected: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_last_actual: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_contract_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    pub screenshot_path: Option<String>,
    pub screenshot_sha256: Option<String>,
    pub elapsed_ms: u64,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewAuiCoordinateEvidence {
    pub presentation_identity: engine_runtime::game_view_presentation::GameViewPresentationIdentity,
    pub target_extent: engine_runtime::game_view_presentation::GameViewExtent,
    pub display_content_rect: engine_runtime::game_view_presentation::GameViewRect,
    pub scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy,
    pub canvas_id: String,
    pub reference_extent: engine_runtime::game_view_presentation::GameViewExtent,
    pub reference_point: engine_runtime::game_view_presentation::GameViewPoint,
    pub target_point: engine_runtime::game_view_presentation::GameViewPoint,
    pub display_point: engine_runtime::game_view_presentation::GameViewPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionAuthorityReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: String,
    pub started_at_epoch_ms: u128,
    pub elapsed_ms: u64,
    pub steps: Vec<ProductionAuthorityStepReport>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProductionAuthorityTerminal {
    Passed,
    Failed { diagnostic: String },
}

fn build_production_authority_terminal_report(
    scenario: &ProductionAuthorityScenario,
    started_at: Option<std::time::Instant>,
    steps: &[ProductionAuthorityStepReport],
    terminal: ProductionAuthorityTerminal,
) -> ProductionAuthorityReport {
    let (status, diagnostics) = match terminal {
        ProductionAuthorityTerminal::Passed => ("passed".to_string(), Vec::new()),
        ProductionAuthorityTerminal::Failed { diagnostic } => {
            ("failed".to_string(), vec![diagnostic])
        }
    };
    let started = started_at.unwrap_or_else(std::time::Instant::now);
    ProductionAuthorityReport {
        schema_version: PRODUCTION_AUTHORITY_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: scenario.scenario_id.clone(),
        status,
        started_at_epoch_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis()),
        elapsed_ms: started.elapsed().as_millis() as u64,
        steps: steps.to_vec(),
        diagnostics,
    }
}

fn persist_production_authority_report(
    scenario: &ProductionAuthorityScenario,
    report: &ProductionAuthorityReport,
) {
    let root = scenario.evidence_root.join(&scenario.scenario_id);
    if std::fs::create_dir_all(&root).is_ok() {
        if let Ok(bytes) = serde_json::to_vec_pretty(report) {
            let _ = std::fs::write(root.join("report.json"), bytes);
        }
    }
}

pub(crate) fn finalize_production_authority_report_once<'a>(
    scenario: Option<&ProductionAuthorityScenario>,
    started_at: Option<std::time::Instant>,
    steps: &[ProductionAuthorityStepReport],
    report: &'a mut Option<ProductionAuthorityReport>,
    terminal: ProductionAuthorityTerminal,
) -> Option<&'a ProductionAuthorityReport> {
    let scenario = scenario?;
    if report.is_none() {
        let terminal_report =
            build_production_authority_terminal_report(scenario, started_at, steps, terminal);
        persist_production_authority_report(scenario, &terminal_report);
        *report = Some(terminal_report);
    }
    report.as_ref()
}

pub(crate) fn ensure_production_authority_terminal_report<'a>(
    scenario: Option<&ProductionAuthorityScenario>,
    started_at: Option<std::time::Instant>,
    steps: &[ProductionAuthorityStepReport],
    report: &'a mut Option<ProductionAuthorityReport>,
    fallback_diagnostic: &str,
) -> Option<&'a ProductionAuthorityReport> {
    finalize_production_authority_report_once(
        scenario,
        started_at,
        steps,
        report,
        ProductionAuthorityTerminal::Failed {
            diagnostic: fallback_diagnostic.to_string(),
        },
    )
}

pub fn production_authority_report_or_fail_closed(
    scenario: &ProductionAuthorityScenario,
    report: Option<ProductionAuthorityReport>,
) -> ProductionAuthorityReport {
    report.unwrap_or_else(|| {
        let report = build_production_authority_terminal_report(
            scenario,
            None,
            &[],
            ProductionAuthorityTerminal::Failed {
                diagnostic: "authority.runner_missing_terminal_report".to_string(),
            },
        );
        persist_production_authority_report(scenario, &report);
        report
    })
}

pub fn production_authority_report_json(
    report: &ProductionAuthorityReport,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::project_observation::ProjectRuntimeObservationSnapshot;

    fn terminal_scenario(scenario_id: &str) -> ProductionAuthorityScenario {
        ProductionAuthorityScenario {
            schema_version: PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
            scenario_id: scenario_id.to_string(),
            evidence_root: std::env::temp_dir().join(format!(
                "aife-production-authority-terminal-{}-{scenario_id}",
                std::process::id()
            )),
            project_root: PathBuf::from("project"),
            recent_project_store_path: PathBuf::from("state/editor_recent_projects.json"),
            workspace_layout_store_root: PathBuf::from("state"),
            physical_width: 1280,
            physical_height: 720,
            game_view_target: None,
            per_step_timeout_ms: 5_000,
            overall_timeout_ms: 30_000,
            steps: Vec::new(),
        }
    }

    fn terminal_step(step_id: &str) -> ProductionAuthorityStepReport {
        ProductionAuthorityStepReport {
            step_id: step_id.to_string(),
            kind: "waitFor".to_string(),
            target: None,
            status: "passed".to_string(),
            actionable: None,
            pointer_down_observed: false,
            pointer_up_observed: false,
            before_command_id: None,
            after_command_id: None,
            runtime_action_id: None,
            runtime_frame_index: None,
            runtime_session_id: None,
            game_view_coordinates: None,
            viewport_input_route: None,
            observation_path: None,
            observation_declared_type: None,
            observation_expected: None,
            observation_last_actual: None,
            observation_contract_id: None,
            timeout_ms: None,
            screenshot_path: None,
            screenshot_sha256: None,
            elapsed_ms: 1,
            diagnostics: Vec::new(),
        }
    }

    fn terminal_report(status: &str, diagnostics: Vec<String>) -> ProductionAuthorityReport {
        ProductionAuthorityReport {
            schema_version: PRODUCTION_AUTHORITY_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: "terminal-contract".to_string(),
            status: status.to_string(),
            started_at_epoch_ms: 1,
            elapsed_ms: 1,
            steps: vec![terminal_step("completed")],
            diagnostics,
        }
    }

    #[test]
    fn production_authority_terminal_close_before_first_step_is_failed() {
        let scenario = terminal_scenario("close-before-first-step");
        let mut report = None;
        finalize_production_authority_report_once(
            Some(&scenario),
            None,
            &[],
            &mut report,
            ProductionAuthorityTerminal::Failed {
                diagnostic: "authority.window_close_requested".to_string(),
            },
        );

        let report = report
            .as_ref()
            .expect("close before first step must produce a report");
        assert_eq!(report.status, "failed");
        assert!(report.steps.is_empty());
        assert_eq!(
            report.diagnostics,
            vec!["authority.window_close_requested".to_string()]
        );
        let _ = std::fs::remove_dir_all(&scenario.evidence_root);
    }

    #[test]
    fn production_authority_terminal_close_preserves_partial_steps() {
        let scenario = terminal_scenario("close-preserves-partial");
        let steps = vec![terminal_step("completed")];
        let mut report = None;
        finalize_production_authority_report_once(
            Some(&scenario),
            Some(std::time::Instant::now()),
            &steps,
            &mut report,
            ProductionAuthorityTerminal::Failed {
                diagnostic: "authority.window_close_requested".to_string(),
            },
        );

        let report = report
            .as_ref()
            .expect("close after a partial run must produce a report");
        assert_eq!(report.steps, steps);
        assert_eq!(report.status, "failed");
        let _ = std::fs::remove_dir_all(&scenario.evidence_root);
    }

    #[test]
    fn production_authority_terminal_exiting_does_not_overwrite_pass() {
        let scenario = terminal_scenario("pass-no-overwrite");
        let passed = terminal_report("passed", Vec::new());
        let mut report = Some(passed.clone());
        ensure_production_authority_terminal_report(
            Some(&scenario),
            None,
            &[],
            &mut report,
            "authority.event_loop_exited_without_terminal_report",
        );
        assert_eq!(report, Some(passed));
    }

    #[test]
    fn production_authority_terminal_exiting_does_not_duplicate_known_failure() {
        let scenario = terminal_scenario("known-failure-no-duplicate");
        let failed = terminal_report(
            "failed",
            vec!["authority.scenario_step_timeout".to_string()],
        );
        let mut report = Some(failed.clone());
        ensure_production_authority_terminal_report(
            Some(&scenario),
            None,
            &[],
            &mut report,
            "authority.event_loop_exited_without_terminal_report",
        );
        assert_eq!(report, Some(failed));
    }

    #[test]
    fn production_authority_terminal_missing_report_is_fail_closed() {
        let scenario = terminal_scenario("runner-missing-report");
        let mut report = None;
        ensure_production_authority_terminal_report(
            Some(&scenario),
            None,
            &[],
            &mut report,
            "authority.runner_missing_terminal_report",
        );

        let report = report
            .as_ref()
            .expect("active scenario runner return must fail closed");
        assert_eq!(report.status, "failed");
        assert_eq!(
            report.diagnostics,
            vec!["authority.runner_missing_terminal_report".to_string()]
        );
        let _ = std::fs::remove_dir_all(&scenario.evidence_root);
    }

    #[test]
    fn production_authority_report_or_fail_closed_preserves_existing_and_builds_missing() {
        let scenario = terminal_scenario("public-fail-closed-helper");
        let passed = terminal_report("passed", Vec::new());
        assert_eq!(
            production_authority_report_or_fail_closed(&scenario, Some(passed.clone())),
            passed
        );

        let failed = production_authority_report_or_fail_closed(&scenario, None);
        assert_eq!(
            failed.schema_version,
            PRODUCTION_AUTHORITY_REPORT_SCHEMA_VERSION
        );
        assert_eq!(failed.scenario_id, scenario.scenario_id);
        assert_eq!(failed.status, "failed");
        assert!(failed.steps.is_empty());
        assert_eq!(
            failed.diagnostics,
            vec!["authority.runner_missing_terminal_report".to_string()]
        );
        let _ = std::fs::remove_dir_all(&scenario.evidence_root);
    }

    #[test]
    fn production_authority_scenario_rejects_unbounded_steps() {
        let scenario = ProductionAuthorityScenario {
            schema_version: PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
            scenario_id: "bounded".to_string(),
            evidence_root: PathBuf::from("evidence"),
            project_root: PathBuf::from("project"),
            recent_project_store_path: PathBuf::from("state/editor_recent_projects.json"),
            workspace_layout_store_root: PathBuf::from("state"),
            physical_width: 1280,
            physical_height: 720,
            game_view_target: None,
            per_step_timeout_ms: 5_000,
            overall_timeout_ms: 30_000,
            steps: (0..=MAX_SCENARIO_STEPS)
                .map(|index| ProductionAuthorityStep::WaitFor {
                    step_id: format!("step-{index}"),
                    timeout_ms: None,
                    condition: ProductionAuthorityCondition::ActiveRuntime { active: true },
                })
                .collect(),
        };

        assert_eq!(
            scenario.validate().unwrap_err(),
            "authority.scenario_step_count_invalid"
        );
    }

    #[test]
    fn production_authority_scenario_accepts_single_session_flow() {
        let scenario = ProductionAuthorityScenario {
            schema_version: PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
            scenario_id: "tower-gate-g".to_string(),
            evidence_root: PathBuf::from("evidence"),
            project_root: PathBuf::from("project"),
            recent_project_store_path: PathBuf::from("state/editor_recent_projects.json"),
            workspace_layout_store_root: PathBuf::from("state"),
            physical_width: 1280,
            physical_height: 720,
            game_view_target: Some(
                engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_720x1280(),
            ),
            per_step_timeout_ms: 5_000,
            overall_timeout_ms: 60_000,
            steps: vec![
                ProductionAuthorityStep::ClickEditorWidget {
                    step_id: "open".to_string(),
                    widget_id: "editor/control/hit.project_launcher.recent.0".to_string(),
                },
                ProductionAuthorityStep::ClickGameViewAuiNode {
                    step_id: "start".to_string(),
                    node_id: "start-button".to_string(),
                    expected_action_id: Some("td.start-round".to_string()),
                },
                ProductionAuthorityStep::Capture {
                    step_id: "capture".to_string(),
                    checkpoint_id: "round-1".to_string(),
                },
            ],
        };

        assert!(scenario.validate().is_ok());
        assert_eq!(
            serde_json::to_value(scenario.game_view_target).unwrap(),
            serde_json::json!({
                "extent": { "width": 720, "height": 1280 },
                "scalePolicy": "contain"
            })
        );
    }

    #[test]
    fn production_authority_wait_conditions_are_schema_stable() {
        assert_eq!(
            serde_json::to_value(
                ProductionAuthorityCondition::RuntimeFrameAdvancedSinceStep {
                    step_id: "round-1".to_string(),
                    minimum_delta: 240,
                }
            )
            .unwrap(),
            serde_json::json!({
                "kind": "runtimeFrameAdvancedSinceStep",
                "stepId": "round-1",
                "minimumDelta": 240
            })
        );
        assert_eq!(
            serde_json::to_value(ProductionAuthorityCondition::GameViewAuiNodeActionable {
                node_id: "primary-action".to_string(),
            })
            .unwrap(),
            serde_json::json!({
                "kind": "gameViewAuiNodeActionable",
                "nodeId": "primary-action"
            })
        );
        assert_eq!(
            serde_json::to_value(ProductionAuthorityStep::WaitFor {
                step_id: "round-2".to_string(),
                timeout_ms: Some(90_000),
                condition: ProductionAuthorityCondition::ProjectValueEquals {
                    path: "sample.round".to_string(),
                    equals: serde_json::json!(2),
                },
            })
            .unwrap(),
            serde_json::json!({
                "kind": "waitFor",
                "stepId": "round-2",
                "timeoutMs": 90000,
                "condition": {
                    "kind": "projectValueEquals",
                    "path": "sample.round",
                    "equals": 2
                }
            })
        );
    }

    fn published_integer_state(runtime_frame: u64, value: i64) -> ProjectRuntimeObservationState {
        ProjectRuntimeObservationState::Published {
            snapshot: ProjectRuntimeObservationSnapshot {
                schema_version: "project-runtime-observation-snapshot.v1".to_string(),
                runtime_frame,
                session_id: "session-a".to_string(),
                contract_id: "test.runtime-observations".to_string(),
                contract_digest: "sha256:test".to_string(),
                declared_types: [("test.round".to_string(), ProjectObservationType::Integer)]
                    .into_iter()
                    .collect(),
                values: [(
                    "test.round".to_string(),
                    ProjectObservationValue::Integer(value),
                )]
                .into_iter()
                .collect(),
            },
        }
    }

    #[test]
    fn production_authority_project_value_evaluation_is_typed_fresh_and_fail_closed() {
        let unavailable = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!(2),
            None,
            Some("session-a"),
            None,
        );
        assert_eq!(
            unavailable.status,
            ProductionAuthorityConditionStatus::Failed
        );
        assert_eq!(
            unavailable.diagnostic_code.as_deref(),
            Some("authority.project_observation_contract_unavailable")
        );

        let state = published_integer_state(5, 1);
        let unknown = evaluate_project_value_condition(
            "test.missing",
            &serde_json::json!(2),
            Some(&state),
            Some("session-a"),
            None,
        );
        assert_eq!(
            unknown.diagnostic_code.as_deref(),
            Some("authority.project_observation_path_unknown")
        );
        let wrong_type = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!("2"),
            Some(&state),
            Some("session-a"),
            None,
        );
        assert_eq!(
            wrong_type.diagnostic_code.as_deref(),
            Some("authority.project_observation_expected_type_mismatch")
        );

        let stale = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!(2),
            Some(&state),
            Some("session-a"),
            Some(5),
        );
        assert_eq!(stale.status, ProductionAuthorityConditionStatus::Pending);
        assert!(stale.last_actual.is_none());
        let mismatch = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!(2),
            Some(&state),
            Some("session-a"),
            Some(4),
        );
        assert_eq!(mismatch.status, ProductionAuthorityConditionStatus::Pending);
        assert_eq!(mismatch.last_actual, Some(serde_json::json!(1)));

        let matched_state = published_integer_state(6, 2);
        let matched = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!(2),
            Some(&matched_state),
            Some("session-a"),
            Some(5),
        );
        assert_eq!(matched.status, ProductionAuthorityConditionStatus::Passed);
        let changed = evaluate_project_value_condition(
            "test.round",
            &serde_json::json!(2),
            Some(&matched_state),
            Some("session-b"),
            None,
        );
        assert_eq!(
            changed.diagnostic_code.as_deref(),
            Some("authority.project_observation_session_changed")
        );
    }

    #[test]
    fn production_authority_wait_timeout_is_bounded_independently_from_default() {
        let mut scenario = ProductionAuthorityScenario {
            schema_version: PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
            scenario_id: "step-timeout".to_string(),
            evidence_root: PathBuf::from("evidence"),
            project_root: PathBuf::from("project"),
            recent_project_store_path: PathBuf::from("state/recent.json"),
            workspace_layout_store_root: PathBuf::from("state"),
            physical_width: 1280,
            physical_height: 720,
            game_view_target: None,
            per_step_timeout_ms: 5_000,
            overall_timeout_ms: 180_000,
            steps: vec![ProductionAuthorityStep::WaitFor {
                step_id: "wait".to_string(),
                timeout_ms: Some(120_000),
                condition: ProductionAuthorityCondition::ProjectValueEquals {
                    path: "test.round".to_string(),
                    equals: serde_json::json!(2),
                },
            }],
        };
        assert!(scenario.validate().is_ok());
        let ProductionAuthorityStep::WaitFor { timeout_ms, .. } = &mut scenario.steps[0] else {
            unreachable!();
        };
        *timeout_ms = Some(120_001);
        assert_eq!(
            scenario.validate().unwrap_err(),
            "authority.scenario_wait_timeout_invalid"
        );
    }
}
