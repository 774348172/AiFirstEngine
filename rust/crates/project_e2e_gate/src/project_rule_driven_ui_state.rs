use editor_core::{
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus,
};
use engine_input::{ActionPhase, ActionSnapshot, InputActionState, InputTraceSummary};
use engine_runtime::aui::{
    AuiBindingValue, AuiRuntimePresenter, AuiSnapshotSource, ProjectUiStateProducerContext,
    ProjectUiStateSnapshotReport,
};
use engine_runtime::component_value::RuntimeValue;
use engine_runtime::components::ComponentTypeId;
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
use engine_runtime::gameplay_rule_report::{
    GameplayRuleRuntimeExecutionReport, GameplayRuleRuntimeExecutionStatus,
};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-project-rule-driven-ui-state-snapshot-report.v1";
pub const COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_SCENARIO_ID: &str =
    "complex-shooter-project-rule-driven-ui-state-snapshot-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterProjectRuleDrivenUiStateStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectRuleDrivenUiStateMetrics {
    pub frames_simulated: u64,
    pub score_after: Option<i64>,
    pub score_text: Option<String>,
    pub score_text_matches_runtime_score: bool,
    pub hp_ratio: Option<f32>,
    pub wave_text: Option<String>,
    pub active_binding_path_count: usize,
    pub produced_path_count: usize,
    pub missing_path_count: usize,
    pub cache_status: String,
    pub dirty_domain_count: usize,
    pub source_path_count: usize,
    pub used_project_producer: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterProjectRuleDrivenUiStateReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterProjectRuleDrivenUiStateStatus,
    pub project_root: String,
    pub output_root: String,
    pub runtime_package_dir: Option<String>,
    pub core_report: Option<GameplayRuleRuntimeExecutionReport>,
    pub ui_state_snapshot_report: Option<ProjectUiStateSnapshotReport>,
    pub metrics: ComplexShooterProjectRuleDrivenUiStateMetrics,
    pub diagnostics: Vec<String>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterProjectRuleDrivenUiStateRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub frames: u64,
}

impl ComplexShooterProjectRuleDrivenUiStateRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            frames: 2,
        }
    }
}

pub fn run_complex_shooter_project_rule_driven_ui_state_report(
    request: ComplexShooterProjectRuleDrivenUiStateRequest,
) -> ComplexShooterProjectRuleDrivenUiStateReport {
    let mut report = ComplexShooterProjectRuleDrivenUiStateReport {
        schema_version: COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_PROJECT_RULE_DRIVEN_UI_STATE_SCENARIO_ID.to_string(),
        status: ComplexShooterProjectRuleDrivenUiStateStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        runtime_package_dir: None,
        core_report: None,
        ui_state_snapshot_report: None,
        metrics: ComplexShooterProjectRuleDrivenUiStateMetrics {
            frames_simulated: request.frames,
            ..ComplexShooterProjectRuleDrivenUiStateMetrics::default()
        },
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
        next_actions: Vec::new(),
    };

    if let Err(error) = fs::create_dir_all(&request.output_root) {
        report
            .diagnostics
            .push(format!("fail:output_root_create_failed:{error}"));
        return report;
    }

    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&request.project_root),
    );
    if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
        report
            .diagnostics
            .push("fail:project_runtime_package_assembly_failed".to_string());
        return finalize_report(&request.output_root, report);
    }

    let input = assembly
        .build_input
        .expect("successful assembly should carry build input");
    let active_scene_id = assembly
        .active_scene_id
        .clone()
        .unwrap_or_else(|| "scene-main".to_string());
    let package_dir = request.output_root.join("runtime_package");
    let build_request = RuntimePackageBuildRequest::dev_desktop(&package_dir, active_scene_id);
    let build_report = RuntimePackageBuilder::build(&build_request, &input);
    report.runtime_package_dir = Some(package_dir.display().to_string());
    report.artifacts.push(package_dir.display().to_string());
    if build_report.status != RuntimePackageBuildStatus::Success {
        report
            .diagnostics
            .push("fail:runtime_package_build_failed".to_string());
        return finalize_report(&request.output_root, report);
    }

    let load = load_runtime_package(&package_dir);
    let Some(package) = load.value else {
        report
            .diagnostics
            .push("fail:runtime_package_load_failed".to_string());
        return finalize_report(&request.output_root, report);
    };

    let linked_modules = crate::complex_shooter_linked_set();
    let bound_runtime = match engine_runtime::project_runtime_module::ProjectRuntimeBootstrap::bind(
        &package,
        &linked_modules,
    ) {
        Ok(bound_runtime) => bound_runtime,
        Err(error) => {
            report.diagnostics.push(format!(
                "fail:project_runtime:{}:{}",
                error.code, error.message
            ));
            return finalize_report(&request.output_root, report);
        }
    };
    let parts = bound_runtime.into_parts();
    let mut producer = parts.ui_state_producer;

    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    if hydration.has_errors() {
        report
            .diagnostics
            .push("fail:runtime_scene_hydration_failed".to_string());
        return finalize_report(&request.output_root, report);
    }

    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    let mut traces = Vec::new();
    for frame in 1..=request.frames {
        let snapshot = if frame == 1 {
            ActionSnapshot::with_actions(
                frame,
                vec![
                    InputActionState::button("action.fire", ActionPhase::Pressed),
                    InputActionState::axis2("action.move", 1.0, 0.0),
                ],
            )
        } else {
            ActionSnapshot::new(frame)
        };
        let input_trace = InputTraceSummary::from_snapshot(Some(&snapshot));
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_action_snapshot(snapshot)
                .with_input_trace_summary(input_trace),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        traces.push(output.runtime_trace);
    }

    let core_report = GameplayRuleRuntimeExecutionReport::from_traces(
        &package.rules,
        request.frames,
        &traces,
        Vec::new(),
    );
    report.metrics.score_after = read_i64_component_field(
        &world,
        "entity-session-state",
        "project.sessionState",
        "score",
    );

    let document_id = package
        .aui_manifest
        .documents
        .first()
        .map(|entry| entry.document_id.as_str());
    let document = document_id.and_then(|document_id| package.aui_documents.get(document_id));
    let active_binding_paths = document
        .into_iter()
        .flat_map(|document| &document.nodes)
        .flat_map(|node| node.binding_refs.iter().map(|binding| binding.path.clone()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut snapshot_output = producer.produce(
        ProjectUiStateProducerContext::new(request.frames + 1, &package, &world)
            .with_active_binding_paths(active_binding_paths.clone()),
    );
    snapshot_output.report.active_binding_paths = active_binding_paths;
    report.metrics.score_text = read_snapshot_string(&snapshot_output, "game.score_text");
    report.metrics.hp_ratio = read_snapshot_number(&snapshot_output, "player.hp_ratio");
    report.metrics.wave_text = read_snapshot_string(&snapshot_output, "game.wave_text");

    if let Some(document) = document {
        let present = AuiRuntimePresenter::present_project_snapshot_with_fonts(
            document,
            snapshot_output,
            &package.font_atlases,
            &package.font_bundles,
        );
        report.ui_state_snapshot_report = present.report.ui_state_snapshot_report.clone();
    }
    if report.ui_state_snapshot_report.is_none() {
        report
            .diagnostics
            .push("fail:aui_present_snapshot_report_missing".to_string());
    }

    if let Some(snapshot_report) = &report.ui_state_snapshot_report {
        report.metrics.active_binding_path_count = snapshot_report.active_binding_paths.len();
        report.metrics.produced_path_count = snapshot_report.produced_paths.len();
        report.metrics.missing_path_count = snapshot_report.missing_paths.len();
        report.metrics.cache_status = snapshot_report.cache_status.clone();
        report.metrics.dirty_domain_count = snapshot_report.dirty_domains.len();
        report.metrics.source_path_count = snapshot_report.source_paths.len();
        report.metrics.used_project_producer = snapshot_report.snapshot_source
            == AuiSnapshotSource::ProjectProducer
            && snapshot_report.producer_id == "complex_shooter_runtime_ui_state";
    }
    if let (Some(score), Some(score_text)) = (
        report.metrics.score_after,
        report.metrics.score_text.as_deref(),
    ) {
        report.metrics.score_text_matches_runtime_score = score_text == format!("SCORE {score:06}");
    }
    report.core_report = Some(core_report);

    finalize_report(&request.output_root, report)
}

fn read_snapshot_string(
    output: &engine_runtime::aui::ProjectUiStateSnapshotOutput,
    path: &str,
) -> Option<String> {
    match output.snapshot.values.get(path)? {
        AuiBindingValue::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn read_snapshot_number(
    output: &engine_runtime::aui::ProjectUiStateSnapshotOutput,
    path: &str,
) -> Option<f32> {
    match output.snapshot.values.get(path)? {
        AuiBindingValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn read_i64_component_field(
    world: &World,
    entity_id: &str,
    component_type: &str,
    field: &str,
) -> Option<i64> {
    let component = world.component_value(
        &engine_runtime::ids::EntityId::from(entity_id),
        &ComponentTypeId::from(component_type),
    )?;
    let engine_runtime::archetype::ComponentValue::Dynamic { value, .. } = component else {
        return None;
    };
    let RuntimeValue::Object(fields) = value else {
        return None;
    };
    match fields.get(field)? {
        RuntimeValue::I64(value) => Some(*value),
        RuntimeValue::F64(value) => Some(*value as i64),
        _ => None,
    }
}

fn finalize_report(
    output_root: &Path,
    mut report: ComplexShooterProjectRuleDrivenUiStateReport,
) -> ComplexShooterProjectRuleDrivenUiStateReport {
    let core_passed = report.core_report.as_ref().is_some_and(|core| {
        core.status == GameplayRuleRuntimeExecutionStatus::Passed
            && core.collision_pair_count > 0
            && core.command_apply_failed_count == 0
    });
    let ui_passed = report.metrics.score_after.unwrap_or(0) > 0
        && report.metrics.score_text_matches_runtime_score
        && report.metrics.used_project_producer
        && report.metrics.active_binding_path_count > 0
        && report.metrics.produced_path_count >= report.metrics.active_binding_path_count
        && report.metrics.missing_path_count == 0;

    if !core_passed {
        report
            .next_actions
            .push("fix_gameplay_rule_core_report".to_string());
    }
    if !ui_passed {
        report
            .next_actions
            .push("fix_project_rule_driven_ui_state_snapshot".to_string());
    }
    report.status = if report.diagnostics.is_empty() && core_passed && ui_passed {
        ComplexShooterProjectRuleDrivenUiStateStatus::Passed
    } else {
        ComplexShooterProjectRuleDrivenUiStateStatus::Failed
    };

    let report_path = output_root
        .join("reports")
        .join("complex-shooter-project-rule-driven-ui-state-snapshot-report.json");
    report.artifacts.push(report_path.display().to_string());
    if let Err(error) = write_json(&report_path, &report) {
        report
            .diagnostics
            .push(format!("fail:report_write_failed:{error}"));
        report.status = ComplexShooterProjectRuleDrivenUiStateStatus::Failed;
    }
    report
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
