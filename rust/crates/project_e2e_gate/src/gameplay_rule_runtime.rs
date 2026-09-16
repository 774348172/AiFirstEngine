use editor_core::{
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblyStatus,
};
use engine_input::{ActionPhase, ActionSnapshot, InputActionState, InputTraceSummary};
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

pub const COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-gameplay-rule-runtime-execution-report.v1";
pub const COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_SCENARIO_ID: &str =
    "complex-shooter-gameplay-rule-runtime-execution-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterGameplayRuleRuntimeStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGameplayRuleRuntimeMetrics {
    pub frames_simulated: u64,
    pub manifest_rule_count: usize,
    pub observed_rule_count: usize,
    pub player_move_write_count: usize,
    pub fire_command_enqueue_count: usize,
    pub bullet_prefab_apply_count: usize,
    pub linear_motion_write_count: usize,
    pub lifetime_write_count: usize,
    pub collision_response_write_count: usize,
    pub collision_pair_count: usize,
    pub score_after: Option<i64>,
    pub score_changed: bool,
    pub enemy_hp_write_count: usize,
    pub session_score_write_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGameplayRuleRuntimeReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterGameplayRuleRuntimeStatus,
    pub project_root: String,
    pub output_root: String,
    pub runtime_package_dir: Option<String>,
    pub core_report: Option<GameplayRuleRuntimeExecutionReport>,
    pub metrics: ComplexShooterGameplayRuleRuntimeMetrics,
    pub diagnostics: Vec<String>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterGameplayRuleRuntimeRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub frames: u64,
}

impl ComplexShooterGameplayRuleRuntimeRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            frames: 2,
        }
    }
}

pub fn run_complex_shooter_gameplay_rule_runtime_report(
    request: ComplexShooterGameplayRuleRuntimeRequest,
) -> ComplexShooterGameplayRuleRuntimeReport {
    let mut report = ComplexShooterGameplayRuleRuntimeReport {
        schema_version: COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_GAMEPLAY_RULE_RUNTIME_SCENARIO_ID.to_string(),
        status: ComplexShooterGameplayRuleRuntimeStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        runtime_package_dir: None,
        core_report: None,
        metrics: ComplexShooterGameplayRuleRuntimeMetrics {
            frames_simulated: request.frames,
            ..ComplexShooterGameplayRuleRuntimeMetrics::default()
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
        for diagnostic in assembly.report.diagnostics {
            report.diagnostics.push(format!(
                "assembly:{:?}:{}:{}",
                diagnostic.domain, diagnostic.code, diagnostic.message
            ));
        }
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
        for diagnostic in build_report.diagnostics {
            report
                .diagnostics
                .push(format!("build:{}:{}", diagnostic.code, diagnostic.message));
        }
        return finalize_report(&request.output_root, report);
    }

    let load = load_runtime_package(&package_dir);
    let Some(package) = load.value else {
        report
            .diagnostics
            .push("fail:runtime_package_load_failed".to_string());
        for issue in load.diagnostics.issues {
            report
                .diagnostics
                .push(format!("package:{}:{}", issue.path, issue.message));
        }
        return finalize_report(&request.output_root, report);
    };

    let linked_modules = crate::complex_shooter_linked_set();
    let project_logic = match engine_runtime::project_runtime_module::ProjectRuntimeBootstrap::bind(
        &package,
        &linked_modules,
    ) {
        Ok(bound_runtime) => bound_runtime.into_project_logic(),
        Err(error) => {
            report.diagnostics.push(format!(
                "fail:project_runtime:{}:{}",
                error.code, error.message
            ));
            return finalize_report(&request.output_root, report);
        }
    };

    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    if hydration.has_errors() {
        report
            .diagnostics
            .push("fail:runtime_scene_hydration_failed".to_string());
        for diagnostic in hydration.instantiate_report.diagnostics {
            report.diagnostics.push(format!(
                "hydrate:{}:{}",
                diagnostic.kind, diagnostic.message
            ));
        }
        return finalize_report(&request.output_root, report);
    }

    let mut host =
        EngineHostLoop::with_project_logic(package.active_scene.id.clone(), project_logic);
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
    apply_metrics_from_traces(&traces, &mut report.metrics);
    report.metrics.manifest_rule_count = core_report.manifest_rule_count;
    report.metrics.observed_rule_count = core_report.observed_rule_ids.len();
    report.metrics.collision_pair_count = core_report.collision_pair_count;
    report.metrics.bullet_prefab_apply_count = *core_report
        .command_apply_by_source
        .get("prefab-player-bullet")
        .unwrap_or(&0);
    report.metrics.score_after = read_i64_component_field(
        &world,
        "entity-session-state",
        "project.sessionState",
        "score",
    );
    report.metrics.score_changed = report.metrics.score_after.unwrap_or(0) > 0;
    report.core_report = Some(core_report);

    finalize_report(&request.output_root, report)
}

fn apply_metrics_from_traces(
    traces: &[engine_runtime::runtime_trace::RuntimeTrace],
    metrics: &mut ComplexShooterGameplayRuleRuntimeMetrics,
) {
    for trace in traces {
        for record in &trace.gameplay_records {
            match (record.rule_id.as_str(), record.operation.as_str()) {
                ("rule.player-move", "write") => metrics.player_move_write_count += 1,
                ("rule.fire-bullet", "command_enqueue") => metrics.fire_command_enqueue_count += 1,
                ("rule.linear-motion", "write") => metrics.linear_motion_write_count += 1,
                ("rule.lifetime-cleanup", "write") => metrics.lifetime_write_count += 1,
                ("rule.collision-response", "write") => {
                    metrics.collision_response_write_count += 1;
                    if record
                        .component_type
                        .as_ref()
                        .is_some_and(|component| component.as_str() == "project.combatState")
                        && record.field_path.as_deref() == Some("hp")
                    {
                        metrics.enemy_hp_write_count += 1;
                    }
                    if record
                        .component_type
                        .as_ref()
                        .is_some_and(|component| component.as_str() == "project.sessionState")
                        && record.field_path.as_deref() == Some("score")
                    {
                        metrics.session_score_write_count += 1;
                    }
                }
                _ => {}
            }
        }
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
    mut report: ComplexShooterGameplayRuleRuntimeReport,
) -> ComplexShooterGameplayRuleRuntimeReport {
    let core_passed = report.core_report.as_ref().is_some_and(|core| {
        core.status == GameplayRuleRuntimeExecutionStatus::Passed
            && core.manifest_rule_count >= 5
            && core.command_apply_failed_count == 0
    });
    let metrics_passed = report.metrics.player_move_write_count > 0
        && report.metrics.fire_command_enqueue_count > 0
        && report.metrics.bullet_prefab_apply_count > 0
        && report.metrics.linear_motion_write_count > 0
        && report.metrics.lifetime_write_count > 0
        && report.metrics.collision_pair_count > 0
        && report.metrics.enemy_hp_write_count > 0
        && report.metrics.session_score_write_count > 0
        && report.metrics.score_changed;
    if !core_passed {
        report
            .next_actions
            .push("fix_gameplay_rule_core_report".to_string());
    }
    if !metrics_passed {
        report
            .next_actions
            .push("fix_complex_shooter_gameplay_rule_evidence".to_string());
    }
    report.status = if report.diagnostics.is_empty() && core_passed && metrics_passed {
        ComplexShooterGameplayRuleRuntimeStatus::Passed
    } else {
        ComplexShooterGameplayRuleRuntimeStatus::Failed
    };

    let report_path = output_root
        .join("reports")
        .join("complex-shooter-gameplay-rule-runtime-execution-report.json");
    report.artifacts.push(report_path.display().to_string());
    if let Err(error) = write_json(&report_path, &report) {
        report
            .diagnostics
            .push(format!("fail:report_write_failed:{error}"));
        report.status = ComplexShooterGameplayRuleRuntimeStatus::Failed;
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
