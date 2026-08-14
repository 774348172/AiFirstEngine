use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::field_path::FieldPath;
use crate::frame_loop::FrameLoop;
use crate::ids::EntityId;
use crate::input_action::{ActionSnapshot, InputTraceSummary};
use crate::render_command::{apply_batch, RenderCommandType};
use crate::render_extract::RenderExtractContext;
use crate::render_state::RenderSceneState;
use crate::runtime_package::{
    RuntimeAssetRef, RuntimeEntity, RuntimeMesh, RuntimeScene, RuntimeTransform, Vector3,
};
use crate::runtime_trace::RuntimeTrace;
use crate::scene_loader::load_scene_into_world;
use crate::{
    archetype::ComponentValue, component_value::RuntimeValue, components::ComponentTypeId,
    project_logic::ProjectLogicRunner, world::World,
};

pub const GOLDEN_SCENARIO_SCHEMA_VERSION: &str = "golden-scenario.v1";

#[derive(Debug, Clone, PartialEq)]
pub struct GoldenScenario {
    pub schema_version: String,
    pub scenario_id: String,
    pub name: String,
    pub fixed_delta_time: f32,
    pub frame_count: u64,
    pub input_frames: Vec<GoldenInputFrame>,
    pub checks: Vec<GoldenCheck>,
}

impl GoldenScenario {
    pub fn new(scenario_id: impl Into<String>, name: impl Into<String>, frame_count: u64) -> Self {
        Self {
            schema_version: GOLDEN_SCENARIO_SCHEMA_VERSION.to_string(),
            scenario_id: scenario_id.into(),
            name: name.into(),
            fixed_delta_time: crate::runtime_time::DEFAULT_FIXED_DELTA_TIME,
            frame_count,
            input_frames: Vec::new(),
            checks: Vec::new(),
        }
    }

    pub fn with_fixed_delta_time(mut self, fixed_delta_time: f32) -> Self {
        self.fixed_delta_time = fixed_delta_time;
        self
    }

    pub fn with_input_frames(mut self, input_frames: Vec<GoldenInputFrame>) -> Self {
        self.input_frames = input_frames;
        self
    }

    pub fn with_checks(mut self, checks: Vec<GoldenCheck>) -> Self {
        self.checks = checks;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoldenInputFrame {
    pub frame_index: u64,
    pub action_snapshot: ActionSnapshot,
}

impl GoldenInputFrame {
    pub fn new(frame_index: u64, action_snapshot: ActionSnapshot) -> Self {
        Self {
            frame_index,
            action_snapshot,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldenExpectedFrame {
    AnyFrame,
    Exact(u64),
    FinalFrame,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoldenCheck {
    pub check_id: String,
    pub expected_frame: GoldenExpectedFrame,
    pub kind: GoldenCheckKind,
}

impl GoldenCheck {
    pub fn new(
        check_id: impl Into<String>,
        expected_frame: GoldenExpectedFrame,
        kind: GoldenCheckKind,
    ) -> Self {
        Self {
            check_id: check_id.into(),
            expected_frame,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoldenCheckKind {
    FrameHashEquals(String),
    EntityExists(EntityId),
    EntityNotExists(EntityId),
    ComponentExists {
        entity_id: EntityId,
        component_type: ComponentTypeId,
    },
    ComponentFieldEquals {
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: FieldPath,
        expected: RuntimeValue,
    },
    TraceEventExists {
        system_id: Option<String>,
        phase: Option<String>,
        message_contains: Option<String>,
    },
    GameplayTraceExists {
        rule_id: Option<String>,
        operation: Option<String>,
        entity_id: Option<EntityId>,
        component_type: Option<ComponentTypeId>,
    },
    Physics2DPairCountEquals(usize),
    RenderProxyCountEquals(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoldenScenarioStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenCheckResult {
    pub check_id: String,
    pub frame_index: u64,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
    pub related_trace_count: usize,
    pub suggested_domain: GoldenFailureDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoldenFailureDomain {
    Input,
    Logic,
    Ecs,
    Physics2D,
    SpawnDespawn,
    RenderExtract,
    Aui,
    Unknown,
}

impl GoldenFailureDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Logic => "logic",
            Self::Ecs => "ecs",
            Self::Physics2D => "physics2d",
            Self::SpawnDespawn => "spawn_despawn",
            Self::RenderExtract => "render_extract",
            Self::Aui => "aui",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenFrameRecord {
    pub frame_index: u64,
    pub frame_hash: String,
    pub entity_count: usize,
    pub trace_event_count: usize,
    pub gameplay_trace_count: usize,
    pub physics2d_trace_count: usize,
    pub physics2d_pair_count: usize,
    pub render_proxy_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenScenarioReport {
    pub scenario_id: String,
    pub status: GoldenScenarioStatus,
    pub frames_run: u64,
    pub first_failed_frame: Option<u64>,
    pub check_results: Vec<GoldenCheckResult>,
    pub frame_records: Vec<GoldenFrameRecord>,
    pub failure_summary: Option<String>,
}

impl GoldenScenarioReport {
    pub fn passed(&self) -> bool {
        self.status == GoldenScenarioStatus::Passed
    }
}

#[derive(Debug, Clone)]
pub struct GoldenScenarioRunner {
    scenario: GoldenScenario,
    project_logic: ProjectLogicRunner,
}

impl GoldenScenarioRunner {
    pub fn new(scenario: GoldenScenario) -> Self {
        Self {
            scenario,
            project_logic: ProjectLogicRunner::empty(),
        }
    }

    pub fn with_project_logic(mut self, project_logic: ProjectLogicRunner) -> Self {
        self.project_logic = project_logic;
        self
    }

    pub fn run(&self, scene_id: impl Into<String>, world: &mut World) -> GoldenScenarioReport {
        let scene_id = scene_id.into();
        let mut frame_loop = FrameLoop::with_project_logic(scene_id, self.project_logic.clone());
        frame_loop
            .runtime_time_mut()
            .set_fixed_delta_time(self.scenario.fixed_delta_time);
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut check_results = Vec::new();
        let mut frame_records = Vec::new();

        for frame_index in 1..=self.scenario.frame_count {
            let action_snapshot = self
                .scenario
                .input_frames
                .iter()
                .find(|input| input.frame_index == frame_index)
                .map(|input| input.action_snapshot.clone());
            let input_summary = action_snapshot
                .as_ref()
                .map(|snapshot| InputTraceSummary::from_snapshot(Some(snapshot)));
            let output = frame_loop.tick_runtime_frame_with_input_and_delta(
                world,
                &mut render_scene,
                &mut extract,
                action_snapshot.as_ref(),
                input_summary,
                self.scenario.fixed_delta_time,
            );

            frame_records.push(GoldenFrameRecord {
                frame_index,
                frame_hash: output.frame_hash.clone(),
                entity_count: world.entity_count(),
                trace_event_count: output.runtime_trace.events.len(),
                gameplay_trace_count: output.runtime_trace.gameplay_records.len(),
                physics2d_trace_count: output.runtime_trace.physics2d_records.len(),
                physics2d_pair_count: output.physics2d_pair_report.pairs.len(),
                render_proxy_count: output.render_scene_proxy_count,
            });

            for check in self.checks_for_frame(frame_index) {
                let result = evaluate_check(
                    check,
                    frame_index,
                    world,
                    &output.runtime_trace,
                    &output.frame_hash,
                    output.physics2d_pair_report.pairs.len(),
                    output.render_scene_proxy_count,
                );
                check_results.push(result);
            }
        }

        for check in self.final_checks() {
            let frame_index = self.scenario.frame_count;
            let Some(record) = frame_records.last() else {
                continue;
            };
            let result = evaluate_final_record_check(check, frame_index, world, record);
            check_results.push(result);
        }

        let first_failure = check_results.iter().find(|result| !result.passed);
        let status = if first_failure.is_some() {
            GoldenScenarioStatus::Failed
        } else {
            GoldenScenarioStatus::Passed
        };
        let failure_summary = first_failure.map(build_failure_summary);

        GoldenScenarioReport {
            scenario_id: self.scenario.scenario_id.clone(),
            status,
            frames_run: self.scenario.frame_count,
            first_failed_frame: first_failure.map(|result| result.frame_index),
            check_results,
            frame_records,
            failure_summary,
        }
    }

    fn checks_for_frame(&self, frame_index: u64) -> Vec<&GoldenCheck> {
        self.scenario
            .checks
            .iter()
            .filter(|check| match check.expected_frame {
                GoldenExpectedFrame::AnyFrame => true,
                GoldenExpectedFrame::Exact(expected) => expected == frame_index,
                GoldenExpectedFrame::FinalFrame => false,
            })
            .collect()
    }

    fn final_checks(&self) -> Vec<&GoldenCheck> {
        self.scenario
            .checks
            .iter()
            .filter(|check| check.expected_frame == GoldenExpectedFrame::FinalFrame)
            .collect()
    }
}

fn evaluate_check(
    check: &GoldenCheck,
    frame_index: u64,
    world: &World,
    trace: &RuntimeTrace,
    frame_hash: &str,
    physics2d_pair_count: usize,
    render_proxy_count: usize,
) -> GoldenCheckResult {
    match &check.kind {
        GoldenCheckKind::FrameHashEquals(expected) => result(
            check,
            frame_index,
            frame_hash == expected,
            format!("frame_hash={expected}"),
            format!("frame_hash={frame_hash}"),
            trace.events.len(),
            GoldenFailureDomain::Unknown,
        ),
        GoldenCheckKind::EntityExists(entity_id) => result(
            check,
            frame_index,
            world.entity(entity_id).is_some(),
            format!("entity_exists={entity_id}"),
            format!("entity_exists={}", world.entity(entity_id).is_some()),
            trace.gameplay_records.len(),
            GoldenFailureDomain::Ecs,
        ),
        GoldenCheckKind::EntityNotExists(entity_id) => result(
            check,
            frame_index,
            world.entity(entity_id).is_none(),
            format!("entity_not_exists={entity_id}"),
            format!("entity_exists={}", world.entity(entity_id).is_some()),
            trace.gameplay_records.len(),
            GoldenFailureDomain::Ecs,
        ),
        GoldenCheckKind::ComponentExists {
            entity_id,
            component_type,
        } => result(
            check,
            frame_index,
            world.component_value(entity_id, component_type).is_some(),
            format!("component_exists={entity_id}.{component_type}"),
            format!(
                "component_exists={}",
                world.component_value(entity_id, component_type).is_some()
            ),
            trace.gameplay_records.len(),
            GoldenFailureDomain::Ecs,
        ),
        GoldenCheckKind::ComponentFieldEquals {
            entity_id,
            component_type,
            field_path,
            expected,
        } => {
            let actual = world
                .component_value(entity_id, component_type)
                .and_then(|value| component_field_value(&value, field_path));
            result(
                check,
                frame_index,
                actual.as_ref() == Some(expected),
                format!(
                    "component_field={} .{} == {}",
                    component_type,
                    field_path.as_str(),
                    runtime_value_summary(expected)
                ),
                format!(
                    "actual={}",
                    actual
                        .as_ref()
                        .map(runtime_value_summary)
                        .unwrap_or_else(|| "missing".to_string())
                ),
                trace.gameplay_records.len(),
                GoldenFailureDomain::Ecs,
            )
        }
        GoldenCheckKind::TraceEventExists {
            system_id,
            phase,
            message_contains,
        } => {
            let matched = trace.events.iter().filter(|event| {
                system_id
                    .as_ref()
                    .is_none_or(|expected| event.system_id == *expected)
                    && phase
                        .as_ref()
                        .is_none_or(|expected| event.phase == *expected)
                    && message_contains
                        .as_ref()
                        .is_none_or(|expected| event.message.contains(expected))
            });
            let count = matched.count();
            result(
                check,
                frame_index,
                count > 0,
                "trace_event_exists".to_string(),
                format!("matched={count}"),
                count,
                trace_domain(system_id, phase, message_contains),
            )
        }
        GoldenCheckKind::GameplayTraceExists {
            rule_id,
            operation,
            entity_id,
            component_type,
        } => {
            let count = trace
                .gameplay_records
                .iter()
                .filter(|record| {
                    rule_id
                        .as_ref()
                        .is_none_or(|expected| record.rule_id == *expected)
                        && operation
                            .as_ref()
                            .is_none_or(|expected| record.operation == *expected)
                        && entity_id
                            .as_ref()
                            .is_none_or(|expected| record.entity_id.as_ref() == Some(expected))
                        && component_type
                            .as_ref()
                            .is_none_or(|expected| record.component_type.as_ref() == Some(expected))
                })
                .count();
            result(
                check,
                frame_index,
                count > 0,
                "gameplay_trace_exists".to_string(),
                format!("matched={count}"),
                count,
                GoldenFailureDomain::Logic,
            )
        }
        GoldenCheckKind::Physics2DPairCountEquals(expected) => result(
            check,
            frame_index,
            physics2d_pair_count == *expected,
            format!("physics2d_pair_count={expected}"),
            format!("physics2d_pair_count={physics2d_pair_count}"),
            trace.physics2d_records.len(),
            GoldenFailureDomain::Physics2D,
        ),
        GoldenCheckKind::RenderProxyCountEquals(expected) => result(
            check,
            frame_index,
            render_proxy_count == *expected,
            format!("render_proxy_count={expected}"),
            format!("render_proxy_count={render_proxy_count}"),
            trace
                .events
                .iter()
                .filter(|event| event.system_id == "engine.render_extract")
                .count(),
            GoldenFailureDomain::RenderExtract,
        ),
    }
}

fn evaluate_final_record_check(
    check: &GoldenCheck,
    frame_index: u64,
    world: &World,
    record: &GoldenFrameRecord,
) -> GoldenCheckResult {
    let empty_trace = RuntimeTrace::new();
    evaluate_check(
        check,
        frame_index,
        world,
        &empty_trace,
        &record.frame_hash,
        record.physics2d_pair_count,
        record.render_proxy_count,
    )
}

fn result(
    check: &GoldenCheck,
    frame_index: u64,
    passed: bool,
    expected: String,
    actual: String,
    related_trace_count: usize,
    suggested_domain: GoldenFailureDomain,
) -> GoldenCheckResult {
    GoldenCheckResult {
        check_id: check.check_id.clone(),
        frame_index,
        passed,
        expected,
        actual,
        related_trace_count,
        suggested_domain,
    }
}

fn build_failure_summary(result: &GoldenCheckResult) -> String {
    format!(
        "GoldenScenario failed: check_id={} frame={} expected=[{}] actual=[{}] domain={} related_trace_count={}",
        result.check_id,
        result.frame_index,
        result.expected,
        result.actual,
        result.suggested_domain.as_str(),
        result.related_trace_count
    )
}

fn trace_domain(
    system_id: &Option<String>,
    phase: &Option<String>,
    message_contains: &Option<String>,
) -> GoldenFailureDomain {
    let text = format!(
        "{} {} {}",
        system_id.as_deref().unwrap_or_default(),
        phase.as_deref().unwrap_or_default(),
        message_contains.as_deref().unwrap_or_default()
    );
    if text.contains("engine.input") || text.contains("Input") {
        GoldenFailureDomain::Input
    } else if text.contains("project.rule") {
        GoldenFailureDomain::Logic
    } else if text.contains("physics2d") || text.contains("Physics2D") {
        GoldenFailureDomain::Physics2D
    } else if text.contains("render_extract") || text.contains("RenderExtract") {
        GoldenFailureDomain::RenderExtract
    } else if text.contains("aui") || text.contains("Aui") {
        GoldenFailureDomain::Aui
    } else {
        GoldenFailureDomain::Unknown
    }
}

fn component_field_value(value: &ComponentValue, field_path: &FieldPath) -> Option<RuntimeValue> {
    match value {
        ComponentValue::Transform(transform) => match field_path.as_str() {
            "local_position" => Some(RuntimeValue::Vec3(transform.local_position)),
            "local_position.x" => Some(RuntimeValue::F64(transform.local_position.x as f64)),
            "local_position.y" => Some(RuntimeValue::F64(transform.local_position.y as f64)),
            "local_position.z" => Some(RuntimeValue::F64(transform.local_position.z as f64)),
            "local_rotation" => Some(RuntimeValue::Vec3(transform.local_rotation)),
            "local_scale" => Some(RuntimeValue::Vec3(transform.local_scale)),
            _ => None,
        },
        ComponentValue::Renderable(renderable) => match field_path.as_str() {
            "visible" => Some(RuntimeValue::Bool(renderable.visible)),
            "layer" => Some(RuntimeValue::String(renderable.layer.clone())),
            "mesh_ref" => renderable.mesh_ref.clone().map(RuntimeValue::AssetRef),
            "material_ref" => renderable.material_ref.clone().map(RuntimeValue::AssetRef),
            _ => None,
        },
        ComponentValue::Dynamic { value, .. } => runtime_value_at_path(value, field_path),
        _ => None,
    }
}

fn runtime_value_at_path(value: &RuntimeValue, field_path: &FieldPath) -> Option<RuntimeValue> {
    let mut current = value;
    for segment in field_path.segments() {
        let RuntimeValue::Object(fields) = current else {
            return None;
        };
        current = fields.get(segment)?;
    }
    Some(current.clone())
}

fn runtime_value_summary(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Null => "null".to_string(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::I64(value) => value.to_string(),
        RuntimeValue::F64(value) => format!("{value:.6}"),
        RuntimeValue::String(value) => value.clone(),
        RuntimeValue::Vec2 { x, y } => format!("vec2({x:.6},{y:.6})"),
        RuntimeValue::Vec3(value) => format!("vec3({:.6},{:.6},{:.6})", value.x, value.y, value.z),
        RuntimeValue::Color { r, g, b, a } => {
            format!("color({r:.6},{g:.6},{b:.6},{a:.6})")
        }
        RuntimeValue::EntityRef(value) => value.to_string(),
        RuntimeValue::AssetRef(value) => value.clone(),
        RuntimeValue::Object(fields) => format!("object({} fields)", fields.len()),
        RuntimeValue::Array(values) => format!("array({} items)", values.len()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldenScenarioId {
    EmptySceneLoad,
    SingleEntityTransform,
    RenderableSnapshot,
    FixedTick10Frames,
    RenderCommandSceneState,
    EngineHostLoopMinimalRenderer,
}

pub fn run_golden_scenario(id: GoldenScenarioId) -> Result<(), String> {
    match id {
        GoldenScenarioId::EmptySceneLoad => {
            let scene = empty_scene();
            let world = load_scene_into_world(&scene)
                .value
                .ok_or("empty scene should load")?;
            if world.entity_count() != 0 {
                return Err(format!("expected 0 entities, got {}", world.entity_count()));
            }
            Ok(())
        }
        GoldenScenarioId::SingleEntityTransform => {
            let scene = single_entity_scene(false);
            let world = load_scene_into_world(&scene)
                .value
                .ok_or("single entity scene should load")?;
            let entity_id = crate::ids::EntityId::from("entity-player");
            let transform = world
                .transform(&entity_id)
                .ok_or("missing player transform")?;
            if transform.local_position.y != 1.0 {
                return Err(format!("expected y=1, got {}", transform.local_position.y));
            }
            Ok(())
        }
        GoldenScenarioId::RenderableSnapshot => {
            let scene = single_entity_scene(true);
            let world = load_scene_into_world(&scene)
                .value
                .ok_or("renderable scene should load")?;
            let mut frame_loop = FrameLoop::new(scene.id.clone());
            let output = frame_loop.tick(&world);
            let renderable = output
                .snapshot
                .renderables
                .first()
                .ok_or("missing renderable snapshot item")?;
            if renderable.mesh_ref.as_deref() != Some("model-player") {
                return Err(format!("unexpected mesh ref: {:?}", renderable.mesh_ref));
            }
            Ok(())
        }
        GoldenScenarioId::FixedTick10Frames => {
            let scene = single_entity_scene(false);
            let world = load_scene_into_world(&scene)
                .value
                .ok_or("fixed tick scene should load")?;
            let mut frame_loop = FrameLoop::new(scene.id.clone());
            let mut last_hash = String::new();
            for expected_frame in 1..=10 {
                let output = frame_loop.tick(&world);
                if output.frame != expected_frame {
                    return Err(format!(
                        "expected frame {}, got {}",
                        expected_frame, output.frame
                    ));
                }
                if output.frame_hash.is_empty() {
                    return Err("frame hash must not be empty".to_string());
                }
                last_hash = output.frame_hash;
            }
            if last_hash.is_empty() {
                return Err("last hash must not be empty".to_string());
            }
            Ok(())
        }
        GoldenScenarioId::RenderCommandSceneState => {
            let scene = single_entity_scene(true);
            let mut world = load_scene_into_world(&scene)
                .value
                .ok_or("render command scene should load")?;
            let mut render_scene = RenderSceneState::new();
            let mut extract = RenderExtractContext::new();
            let entity_id = EntityId::from("entity-player");

            world.insert_renderable(
                entity_id.clone(),
                crate::components::Renderable {
                    mesh_ref: Some("model-player".to_string()),
                    material_ref: Some("material-player".to_string()),
                    visible: true,
                    layer: "default".to_string(),
                },
            );
            let mut add_queue = extract.extract_world_dirty(1, &mut world, &render_scene);
            let add_commands = add_queue.normalize_merge(&render_scene);
            if add_commands.len() != 1
                || add_commands[0].command_type != RenderCommandType::AddProxy
            {
                return Err("expected one AddProxy command".to_string());
            }
            let add_diagnostics = apply_batch(&mut render_scene, &add_commands);
            if !add_diagnostics.is_empty() || render_scene.proxies_len() != 1 {
                return Err("AddProxy should create exactly one render proxy".to_string());
            }

            world.insert_transform(
                entity_id.clone(),
                crate::components::Transform {
                    local_position: crate::math::Vec3 {
                        x: 3.0,
                        y: 1.0,
                        z: 2.0,
                    },
                    local_rotation: crate::math::Vec3::ZERO,
                    local_scale: crate::math::Vec3::ONE,
                },
            );
            let mut update_queue = extract.extract_world_dirty(2, &mut world, &render_scene);
            let update_commands = update_queue.normalize_merge(&render_scene);
            if update_commands.len() != 1
                || update_commands[0].command_type != RenderCommandType::UpdateTransform
            {
                return Err("expected one UpdateTransform command".to_string());
            }
            let update_diagnostics = apply_batch(&mut render_scene, &update_commands);
            let proxy_id = render_scene
                .proxy_for_source(&entity_id)
                .ok_or("missing proxy after update")?;
            let proxy = render_scene.proxy(proxy_id).ok_or("missing proxy state")?;
            if !update_diagnostics.is_empty() || proxy.common.transform.local_position.x != 3.0 {
                return Err("UpdateTransform should update render proxy transform".to_string());
            }

            world.remove_renderable(&entity_id);
            let mut remove_queue = extract.extract_world_dirty(3, &mut world, &render_scene);
            let remove_commands = remove_queue.normalize_merge(&render_scene);
            if remove_commands.len() != 1
                || remove_commands[0].command_type != RenderCommandType::RemoveProxy
            {
                return Err("expected one RemoveProxy command".to_string());
            }
            let remove_diagnostics = apply_batch(&mut render_scene, &remove_commands);
            if !remove_diagnostics.is_empty() || render_scene.proxies_len() != 0 {
                return Err("RemoveProxy should remove render proxy".to_string());
            }
            Ok(())
        }
        GoldenScenarioId::EngineHostLoopMinimalRenderer => {
            let scene = single_entity_scene(true);
            let mut world = load_scene_into_world(&scene)
                .value
                .ok_or("engine host scene should load")?;
            let entity_id = EntityId::from("entity-player");
            world.insert_renderable(
                entity_id,
                crate::components::Renderable {
                    mesh_ref: Some("model-player".to_string()),
                    material_ref: Some("material-player".to_string()),
                    visible: true,
                    layer: "default".to_string(),
                },
            );
            let mut host = EngineHostLoop::new(scene.id.clone());
            let output = host.tick(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut world,
            );

            if !output.runtime_advanced || !output.render_built {
                return Err("engine host should advance runtime and build render data".to_string());
            }
            if output.frame_hash.as_deref().unwrap_or_default().is_empty() {
                return Err("frame hash must not be empty".to_string());
            }
            let report = output
                .render_frame_report
                .as_ref()
                .ok_or("missing render frame report")?;
            if report.counters.applied_command_count != 1 {
                return Err(format!(
                    "expected one applied command, got {}",
                    report.counters.applied_command_count
                ));
            }
            let feature_frame = output
                .renderer_feature_frame
                .as_ref()
                .ok_or("missing renderer feature frame")?;
            if feature_frame.draw_items.len() != 1 {
                return Err(format!(
                    "expected one draw item, got {}",
                    feature_frame.draw_items.len()
                ));
            }
            let minimal_frame = output
                .minimal_renderer_frame
                .as_ref()
                .ok_or("missing minimal renderer frame")?;
            if minimal_frame.draw_record_count != 1 {
                return Err(format!(
                    "expected one draw record, got {}",
                    minimal_frame.draw_record_count
                ));
            }
            if host.render_scene().proxies_len() != 1 {
                return Err(format!(
                    "expected one render proxy, got {}",
                    host.render_scene().proxies_len()
                ));
            }
            Ok(())
        }
    }
}

fn empty_scene() -> RuntimeScene {
    RuntimeScene {
        schema_version: "runtime-scene.v1".to_string(),
        id: "scene-empty".to_string(),
        name: "Empty".to_string(),
        gravity: 0.0,
        background: "#000000".to_string(),
        sky_color: "#111111".to_string(),
        entities: Vec::new(),
    }
}

fn single_entity_scene(with_mesh: bool) -> RuntimeScene {
    RuntimeScene {
        schema_version: "runtime-scene.v1".to_string(),
        id: "scene-main".to_string(),
        name: "Main".to_string(),
        gravity: 0.0,
        background: "#000000".to_string(),
        sky_color: "#111111".to_string(),
        entities: vec![RuntimeEntity {
            schema_version: "runtime-entity.v1".to_string(),
            id: "entity-player".to_string(),
            name: "Player".to_string(),
            kind: "player".to_string(),
            enabled: true,
            parent_id: None,
            sibling_order: 0,
            transform: Some(RuntimeTransform {
                local_position: Vector3 {
                    x: 0.0,
                    y: 1.0,
                    z: 2.0,
                },
                local_rotation: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                local_scale: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }),
            mesh: with_mesh.then(|| RuntimeMesh {
                primitive: Some("model".to_string()),
                color: None,
                label: None,
                asset_ref: Some(RuntimeAssetRef {
                    id: "model-player".to_string(),
                    asset_type: "model".to_string(),
                    guid: None,
                    sub_asset: None,
                }),
                material_ref: Some(RuntimeAssetRef {
                    id: "material-player".to_string(),
                    asset_type: "material".to_string(),
                    guid: None,
                    sub_asset: None,
                }),
                texture_ref: None,
                visible: true,
                layer: "default".to_string(),
                metalness: None,
                roughness: None,
            }),
            sprite_renderer2d: None,
            animator2d: None,
            components: Vec::new(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_value::RuntimeValue;
    use crate::components::{ComponentTypeId, Hierarchy, Transform};
    use crate::input_action::{ActionPhase, InputActionState};
    use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
    use crate::project_logic::{RuleCall, RuleExecutionPlan};

    const GOLDEN_MOVE_RULE: &str = "project.golden_move";

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn golden_world() -> World {
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-source"),
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        world.insert_dynamic_component(
            EntityId::from("entity-source"),
            ComponentTypeId::from("project.marker"),
            RuntimeValue::object([("count", RuntimeValue::I64(1))]),
        );
        world.take_dirty_records();
        world
    }

    fn golden_move_rule(context: &mut LogicContext<'_>) -> LogicResult {
        if !context.action_pressed("action.apply") {
            return LogicResult::skipped(GOLDEN_MOVE_RULE, ExecutorKind::RustAot);
        }
        let entity_id = EntityId::from("entity-source");
        let write = context
            .write_component_field(
                entity_id,
                ComponentTypeId::from("project.marker"),
                &FieldPath::parse("count").unwrap(),
                RuntimeValue::I64(2),
            )
            .expect("write should succeed");
        let mut result = LogicResult::applied(GOLDEN_MOVE_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn golden_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(GOLDEN_MOVE_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(GOLDEN_MOVE_RULE, golden_move_rule);
        runner
    }

    #[test]
    fn golden_empty_scene_load() {
        run_golden_scenario(GoldenScenarioId::EmptySceneLoad).unwrap();
    }

    #[test]
    fn golden_single_entity_transform() {
        run_golden_scenario(GoldenScenarioId::SingleEntityTransform).unwrap();
    }

    #[test]
    fn golden_renderable_snapshot() {
        run_golden_scenario(GoldenScenarioId::RenderableSnapshot).unwrap();
    }

    #[test]
    fn golden_fixed_tick_10_frames() {
        run_golden_scenario(GoldenScenarioId::FixedTick10Frames).unwrap();
    }

    #[test]
    fn golden_render_command_scene_state() {
        run_golden_scenario(GoldenScenarioId::RenderCommandSceneState).unwrap();
    }

    #[test]
    fn golden_engine_host_loop_minimal_renderer() {
        run_golden_scenario(GoldenScenarioId::EngineHostLoopMinimalRenderer).unwrap();
    }

    #[test]
    fn golden_scenario_runner_passes_key_flow_checks() {
        let scenario = GoldenScenario::new("scenario-key-flow", "Key Flow", 2)
            .with_input_frames(vec![GoldenInputFrame::new(
                1,
                ActionSnapshot::with_actions(
                    1,
                    vec![InputActionState::button(
                        "action.apply",
                        ActionPhase::Pressed,
                    )],
                ),
            )])
            .with_checks(vec![
                GoldenCheck::new(
                    "entity-exists",
                    GoldenExpectedFrame::FinalFrame,
                    GoldenCheckKind::EntityExists(EntityId::from("entity-source")),
                ),
                GoldenCheck::new(
                    "marker-updated",
                    GoldenExpectedFrame::FinalFrame,
                    GoldenCheckKind::ComponentFieldEquals {
                        entity_id: EntityId::from("entity-source"),
                        component_type: ComponentTypeId::from("project.marker"),
                        field_path: FieldPath::parse("count").unwrap(),
                        expected: RuntimeValue::I64(2),
                    },
                ),
                GoldenCheck::new(
                    "logic-trace",
                    GoldenExpectedFrame::Exact(1),
                    GoldenCheckKind::GameplayTraceExists {
                        rule_id: Some(GOLDEN_MOVE_RULE.to_string()),
                        operation: Some("write".to_string()),
                        entity_id: Some(EntityId::from("entity-source")),
                        component_type: Some(ComponentTypeId::from("project.marker")),
                    },
                ),
                GoldenCheck::new(
                    "input-trace",
                    GoldenExpectedFrame::Exact(1),
                    GoldenCheckKind::TraceEventExists {
                        system_id: Some("engine.input".to_string()),
                        phase: Some("InputSnapshotReady".to_string()),
                        message_contains: Some("action.apply".to_string()),
                    },
                ),
            ]);
        let mut world = golden_world();

        let report = GoldenScenarioRunner::new(scenario)
            .with_project_logic(golden_runner())
            .run("scene-golden", &mut world);

        assert!(report.passed(), "{report:?}");
        assert_eq!(report.frames_run, 2);
        assert_eq!(report.check_results.len(), 4);
        assert!(report.failure_summary.is_none());
    }

    #[test]
    fn golden_scenario_runner_reports_first_failed_frame() {
        let scenario =
            GoldenScenario::new("scenario-fail", "Fail", 1).with_checks(vec![GoldenCheck::new(
                "missing-entity",
                GoldenExpectedFrame::Exact(1),
                GoldenCheckKind::EntityExists(EntityId::from("entity-missing")),
            )]);
        let mut world = golden_world();

        let report = GoldenScenarioRunner::new(scenario).run("scene-golden", &mut world);

        assert_eq!(report.status, GoldenScenarioStatus::Failed);
        assert_eq!(report.first_failed_frame, Some(1));
        assert!(report
            .failure_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("missing-entity")));
        assert_eq!(
            report.check_results[0].suggested_domain,
            GoldenFailureDomain::Ecs
        );
    }

    #[test]
    fn golden_scenario_runner_supports_trace_checks() {
        let scenario = GoldenScenario::new("scenario-trace", "Trace", 1).with_checks(vec![
            GoldenCheck::new(
                "render-extract-trace",
                GoldenExpectedFrame::Exact(1),
                GoldenCheckKind::TraceEventExists {
                    system_id: Some("engine.render_extract".to_string()),
                    phase: None,
                    message_contains: Some("commands_applied".to_string()),
                },
            ),
            GoldenCheck::new(
                "physics-pair-count",
                GoldenExpectedFrame::Exact(1),
                GoldenCheckKind::Physics2DPairCountEquals(0),
            ),
        ]);
        let mut world = golden_world();

        let report = GoldenScenarioRunner::new(scenario).run("scene-golden", &mut world);

        assert!(report.passed(), "{report:?}");
        assert!(report.frame_records[0].trace_event_count >= 1);
        assert_eq!(report.frame_records[0].physics2d_pair_count, 0);
    }

    #[test]
    fn golden_scenario_runner_keeps_engine_terms_generic() {
        let scenario = GoldenScenario::new("scenario-generic", "Generic", 1).with_checks(vec![
            GoldenCheck::new(
                "entity-exists",
                GoldenExpectedFrame::Exact(1),
                GoldenCheckKind::EntityExists(EntityId::from("entity-source")),
            ),
        ]);
        let mut world = golden_world();

        let report = GoldenScenarioRunner::new(scenario).run("scene-golden", &mut world);
        let text = format!("{report:?}").to_lowercase();

        for forbidden in [
            "enemy",
            "bullet",
            "damage",
            "health",
            "score",
            "wave",
            "skill",
            "weapon",
            "inventory",
            "quest",
            "boss",
        ] {
            assert!(!text.contains(forbidden), "report contains {forbidden}");
        }
    }
}
