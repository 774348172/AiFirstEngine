use crate::gameplay_command::{
    apply_gameplay_commands, apply_gameplay_commands_with_runtime, RuntimeCommandContext,
};
use crate::input_action::ActionSnapshot;
use crate::logic_executor::{
    ExecutorKind, IrInterpreterExecutor, LogicContext, LogicExecutor, LogicResult, RulePhase,
    RustAotExecutor, RustAotRule,
};
use crate::physics2d::CollisionPair;
use crate::runtime_time::{TimeContext, DEFAULT_FIXED_DELTA_TIME};
use crate::runtime_trace::RuntimeTrace;
use crate::world::World;
use crate::world_api::WorldWriteApi;
use crate::{runtime_instance_loader::RuntimeInstanceLoader, runtime_package::RuntimePackage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuleExecutorHint {
    RustAot,
    IrInterpreter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCall {
    pub rule_id: String,
    pub(crate) executor_hint: RuleExecutorHint,
    pub enabled: bool,
}

impl RuleCall {
    pub fn rust_aot(rule_id: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_hint: RuleExecutorHint::RustAot,
            enabled: true,
        }
    }

    pub(crate) fn validation_only_ir_interpreter(rule_id: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_hint: RuleExecutorHint::IrInterpreter,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleExecutionPlan {
    pub fixed_update: Vec<RuleCall>,
    pub frame_update: Vec<RuleCall>,
    pub post_physics: Vec<RuleCall>,
    pub event_handler: Vec<RuleCall>,
}

impl RuleExecutionPlan {
    pub fn empty() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug)]
pub struct ProjectLogicRunner {
    plan: RuleExecutionPlan,
    rust_aot: RustAotExecutor,
    ir_interpreter: IrInterpreterExecutor,
    time_context: TimeContext,
}

impl Default for ProjectLogicRunner {
    fn default() -> Self {
        Self {
            plan: RuleExecutionPlan::empty(),
            rust_aot: RustAotExecutor::new(),
            ir_interpreter: IrInterpreterExecutor::new(),
            time_context: TimeContext::from_delta(0, DEFAULT_FIXED_DELTA_TIME, false),
        }
    }
}

impl ProjectLogicRunner {
    pub fn new(plan: RuleExecutionPlan) -> Self {
        Self {
            plan,
            ..Self::default()
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_rule_manifest_and_registry(
        manifest: &crate::runtime_package::RuntimeRuleManifest,
        registry: &crate::rule_registry::RuleModuleRegistry,
    ) -> Result<Self, crate::rule_registry::RuleRegistryError> {
        registry.build_runner(manifest)
    }

    pub fn plan(&self) -> &RuleExecutionPlan {
        &self.plan
    }

    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.time_context = TimeContext::from_delta(0, delta_time, false);
    }

    pub fn set_time_context(&mut self, time_context: TimeContext) {
        self.time_context = time_context;
    }

    pub fn time_context(&self) -> &TimeContext {
        &self.time_context
    }

    pub fn register_rust_aot_rule(
        &mut self,
        rule_id: impl Into<String>,
        rule: impl for<'a> Fn(&mut LogicContext<'a>) -> LogicResult + Send + Sync + 'static,
    ) {
        self.rust_aot.register_rule(rule_id, RustAotRule::new(rule));
    }

    pub(crate) fn register_rust_aot_rule_value(
        &mut self,
        rule_id: impl Into<String>,
        rule: RustAotRule,
    ) {
        self.rust_aot.register_rule(rule_id, rule);
    }

    pub fn run_fixed_update(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
    ) -> Vec<LogicResult> {
        self.run_fixed_update_with_input(frame_index, world, trace, None)
    }

    pub fn run_fixed_update_with_input(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
    ) -> Vec<LogicResult> {
        self.run_fixed_update_with_time_context(frame_index, world, trace, action_snapshot, None)
    }

    pub fn run_fixed_update_with_time_context(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FixedUpdate,
            &self.plan.fixed_update,
            world,
            trace,
            action_snapshot,
            time_context,
            &[],
            None,
        )
    }

    pub fn run_fixed_update_with_runtime(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        package: &RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FixedUpdate,
            &self.plan.fixed_update,
            world,
            trace,
            action_snapshot,
            Some(self.time_context),
            &[],
            Some(RuntimeCommandContext {
                package,
                instance_loader,
            }),
        )
    }

    pub fn run_fixed_update_with_runtime_and_time_context(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
        package: &RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FixedUpdate,
            &self.plan.fixed_update,
            world,
            trace,
            action_snapshot,
            time_context,
            &[],
            Some(RuntimeCommandContext {
                package,
                instance_loader,
            }),
        )
    }

    pub fn run_frame_update(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
    ) -> Vec<LogicResult> {
        self.run_frame_update_with_input(frame_index, world, trace, None)
    }

    pub fn run_frame_update_with_input(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
    ) -> Vec<LogicResult> {
        self.run_frame_update_with_time_context(frame_index, world, trace, action_snapshot, None)
    }

    pub fn run_frame_update_with_time_context(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FrameUpdate,
            &self.plan.frame_update,
            world,
            trace,
            action_snapshot,
            time_context,
            &[],
            None,
        )
    }

    pub fn run_frame_update_with_runtime(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        package: &RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FrameUpdate,
            &self.plan.frame_update,
            world,
            trace,
            action_snapshot,
            Some(self.time_context),
            &[],
            Some(RuntimeCommandContext {
                package,
                instance_loader,
            }),
        )
    }

    pub fn run_frame_update_with_runtime_and_time_context(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
        package: &RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::FrameUpdate,
            &self.plan.frame_update,
            world,
            trace,
            action_snapshot,
            time_context,
            &[],
            Some(RuntimeCommandContext {
                package,
                instance_loader,
            }),
        )
    }

    pub fn run_post_physics_with_runtime_and_time_context(
        &self,
        frame_index: u64,
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
        collision_pairs: &[CollisionPair],
        package: &RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> Vec<LogicResult> {
        self.run_phase(
            frame_index,
            RulePhase::PostPhysics,
            &self.plan.post_physics,
            world,
            trace,
            action_snapshot,
            time_context,
            collision_pairs,
            Some(RuntimeCommandContext {
                package,
                instance_loader,
            }),
        )
    }

    fn run_phase(
        &self,
        frame_index: u64,
        phase: RulePhase,
        calls: &[RuleCall],
        world: &mut World,
        trace: &mut RuntimeTrace,
        action_snapshot: Option<&ActionSnapshot>,
        time_context: Option<TimeContext>,
        collision_pairs: &[CollisionPair],
        runtime_context: Option<RuntimeCommandContext<'_>>,
    ) -> Vec<LogicResult> {
        let mut results = Vec::new();
        let mut pending_commands = Vec::new();
        let mut phase_time_context = time_context.unwrap_or(self.time_context);
        phase_time_context.in_fixed_step = phase == RulePhase::FixedUpdate;
        if phase == RulePhase::FixedUpdate {
            phase_time_context.delta_time =
                phase_time_context.fixed_delta_time * phase_time_context.time_scale.max(0.0);
        }
        for call in calls {
            let result = if call.enabled {
                let world_api = WorldWriteApi::new(world);
                let mut context = LogicContext::with_time_context(
                    frame_index,
                    phase_time_context,
                    phase,
                    world_api,
                )
                .with_action_snapshot(action_snapshot)
                .with_collision_pairs(collision_pairs);
                let mut result = match call.executor_hint {
                    RuleExecutorHint::RustAot => self.rust_aot.run(&call.rule_id, &mut context),
                    RuleExecutorHint::IrInterpreter => {
                        self.ir_interpreter.run(&call.rule_id, &mut context)
                    }
                };
                let commands = context.take_commands();
                result.queries.extend(context.take_queries());
                result.reads.extend(context.take_reads());
                result
                    .command_ids
                    .extend(commands.iter().map(|(command_id, _)| *command_id));
                trace.record_command_enqueues(
                    frame_index,
                    phase.as_str(),
                    &call.rule_id,
                    &commands,
                );
                pending_commands.extend(commands);
                result
            } else {
                LogicResult::skipped(&call.rule_id, executor_kind(call.executor_hint))
            };
            trace.record_logic_result(frame_index, phase.as_str(), &result);
            results.push(result);
        }
        let apply_records = if let Some(runtime_context) = runtime_context {
            apply_gameplay_commands_with_runtime(world, pending_commands, runtime_context)
        } else {
            apply_gameplay_commands(world, pending_commands)
        };
        trace.record_command_apply_records(frame_index, phase.as_str(), &apply_records);
        results
    }
}

fn executor_kind(hint: RuleExecutorHint) -> ExecutorKind {
    match hint {
        RuleExecutorHint::RustAot => ExecutorKind::RustAot,
        RuleExecutorHint::IrInterpreter => ExecutorKind::IrInterpreter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ComponentTypeId, Hierarchy, Transform};
    use crate::ids::EntityId;
    use crate::logic_executor::{LogicResult, LogicStatus};
    use crate::math::Vec3;
    use crate::runtime_trace::RuntimeTrace;
    use crate::world::DirtyType;

    const MOVE_X_RULE: &str = "project.move_x";
    const MOVE_Y_RULE: &str = "project.move_y";
    const TIME_CONTEXT_RULE: &str = "project.time_context";

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn world_with_transform() -> World {
        let mut world = World::new();
        let entity_id = EntityId::from("entity-player");
        world.spawn_entity(entity_id.clone(), "Player", "actor", true, hierarchy());
        world.insert_transform(entity_id, Transform::identity());
        world.take_dirty_records();
        world
    }

    fn move_x_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform should exist");
        position.x += 1.0;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write should succeed");
        let mut result = LogicResult::applied(MOVE_X_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn move_y_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform should exist");
        position.y = position.x + 2.0;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write should succeed");
        let mut result = LogicResult::applied(MOVE_Y_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn query_transform_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let ids = context.query(crate::query::QuerySpec::all([ComponentTypeId::transform()]));
        let mut result = LogicResult::applied("project.query_transform", ExecutorKind::RustAot);
        result.writes.push(crate::logic_executor::LogicWrite {
            entity_id: ids[0].clone(),
            component_type: ComponentTypeId::from("project.query_result"),
            field: "first_entity".to_string(),
            before: None,
            after: Some(ids[0].to_string()),
        });
        result
    }

    fn spawn_entity_command_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let component_type = ComponentTypeId::from("project.created_marker");
        context.enqueue_command(crate::gameplay_command::GameplayCommand::SpawnEntity {
            entity_id: EntityId::from("entity-created"),
            name: "Created".to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: None,
            components: vec![crate::archetype::ComponentValue::Dynamic {
                component_type,
                value: crate::component_value::RuntimeValue::I64(1),
            }],
        });
        LogicResult::applied("project.spawn_entity", ExecutorKind::RustAot)
    }

    fn instantiate_prefab_rule(context: &mut LogicContext<'_>) -> LogicResult {
        context.request_instantiate_prefab(asset_ref("prefab-ship", "prefab"), None, None);
        LogicResult::applied("project.instantiate_prefab", ExecutorKind::RustAot)
    }

    fn despawn_prefab_instance_rule(context: &mut LogicContext<'_>) -> LogicResult {
        context.request_despawn_prefab_instance(crate::runtime_instance::RuntimeInstanceId(1));
        LogicResult::applied("project.despawn_prefab", ExecutorKind::RustAot)
    }

    fn generic_access_foundation_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let marker = ComponentTypeId::from("project.marker");
        let ids = context.query(crate::query::QuerySpec::all([
            ComponentTypeId::transform(),
            marker.clone(),
        ]));
        let entity_id = ids[0].clone();
        let _component = context
            .read_component(&entity_id, &marker)
            .expect("marker should exist");
        let write = context
            .write_component_field(
                entity_id,
                marker.clone(),
                &crate::field_path::FieldPath::parse("count").unwrap(),
                crate::component_value::RuntimeValue::I64(2),
            )
            .expect("write should succeed");
        context.enqueue_command(crate::gameplay_command::GameplayCommand::SpawnEntity {
            entity_id: EntityId::from("entity-created"),
            name: "Created".to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: None,
            components: vec![crate::archetype::ComponentValue::Dynamic {
                component_type: ComponentTypeId::from("project.created_marker"),
                value: crate::component_value::RuntimeValue::I64(1),
            }],
        });
        let mut result =
            LogicResult::applied("project.generic_access_foundation", ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn time_context_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let mut result = LogicResult::applied(TIME_CONTEXT_RULE, ExecutorKind::RustAot);
        result.writes.push(crate::logic_executor::LogicWrite {
            entity_id: EntityId::from("entity-time"),
            component_type: ComponentTypeId::from("project.time_sample"),
            field: "delta_time".to_string(),
            before: None,
            after: Some(context.time().delta_time.to_string()),
        });
        result
    }

    #[test]
    fn project_logic_runner_executes_frame_update_rule() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(MOVE_X_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(MOVE_X_RULE, move_x_rule);
        let mut world = world_with_transform();
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, LogicStatus::Applied);
        assert_eq!(
            world
                .transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            1.0
        );
    }

    #[test]
    fn project_logic_write_transform_marks_dirty() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(MOVE_X_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(MOVE_X_RULE, move_x_rule);
        let mut world = world_with_transform();
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(world.dirty_records().len(), 1);
        assert_eq!(world.dirty_records()[0].dirty_type, DirtyType::Transform);
    }

    #[test]
    fn rule_execution_plan_runs_in_array_order() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![
                RuleCall::rust_aot(MOVE_X_RULE),
                RuleCall::rust_aot(MOVE_Y_RULE),
            ],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(MOVE_X_RULE, move_x_rule);
        runner.register_rust_aot_rule(MOVE_Y_RULE, move_y_rule);
        let mut world = world_with_transform();
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);

        let position = world
            .transform(&EntityId::from("entity-player"))
            .unwrap()
            .local_position;
        assert_eq!(
            position,
            Vec3 {
                x: 1.0,
                y: 3.0,
                z: 0.0,
            }
        );
    }

    #[test]
    fn project_logic_trace_records_rule_write() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(MOVE_X_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(MOVE_X_RULE, move_x_rule);
        let mut world = world_with_transform();
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(trace.events.len(), 1);
        assert_eq!(trace.events[0].system_id, "project.rule.project.move_x");
        assert_eq!(trace.events[0].phase, "Update");
        assert!(trace.events[0]
            .message
            .contains("RustAotExecutor applied engine.transform.local_position"));
    }

    #[test]
    fn validation_only_ir_interpreter_executor_is_structured_unsupported_in_v1() {
        let runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::validation_only_ir_interpreter(
                "project.hotfix_move",
            )],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        let mut world = world_with_transform();
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(results[0].status, LogicStatus::Unsupported);
        assert_eq!(results[0].errors[0].code, "unsupported_executor");
        assert!(trace.events[0]
            .message
            .contains("ValidationOnlyIrInterpreter unsupported"));
    }

    #[test]
    fn logic_context_query_returns_stable_entity_ids() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.query_transform")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule("project.query_transform", query_transform_rule);
        let mut world = World::new();
        world.spawn_with_components(
            EntityId::from("entity-b"),
            "B",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        world.spawn_with_components(
            EntityId::from("entity-a"),
            "A",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(results[0].writes[0].after, Some("entity-a".to_string()));
    }

    #[test]
    fn command_buffer_spawn_entity_applies_after_rule_execution() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.spawn_entity")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule("project.spawn_entity", spawn_entity_command_rule);
        let mut world = World::new();
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(
            results[0].command_ids,
            vec![crate::gameplay_command::GameplayCommandId(0)]
        );
        assert!(world.entity(&EntityId::from("entity-created")).is_some());
        assert!(trace
            .gameplay_records
            .iter()
            .any(|record| record.operation == "command_apply"
                && record.entity_id == Some(EntityId::from("entity-created"))));
    }

    #[test]
    fn project_logic_runner_supports_generic_query_read_write_command_trace() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.generic_access_foundation")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(
            "project.generic_access_foundation",
            generic_access_foundation_rule,
        );
        let mut world = World::new();
        let marker = ComponentTypeId::from("project.marker");
        let entity_id = EntityId::from("entity-source");
        world.spawn_with_components(
            entity_id.clone(),
            "Source",
            "actor",
            true,
            hierarchy(),
            Some(Transform::identity()),
            None,
        );
        world.insert_dynamic_component(
            entity_id.clone(),
            marker.clone(),
            crate::component_value::RuntimeValue::object([(
                "count",
                crate::component_value::RuntimeValue::I64(1),
            )]),
        );
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(
            world.component_value(&entity_id, &marker),
            Some(crate::archetype::ComponentValue::Dynamic {
                component_type: marker,
                value: crate::component_value::RuntimeValue::object([(
                    "count",
                    crate::component_value::RuntimeValue::I64(2),
                )]),
            })
        );
        assert!(world.entity(&EntityId::from("entity-created")).is_some());
        for operation in ["query", "read", "write", "command_enqueue", "command_apply"] {
            assert!(
                trace
                    .gameplay_records
                    .iter()
                    .any(|record| record.operation == operation),
                "missing gameplay trace operation {operation}"
            );
        }
    }

    #[test]
    fn logic_context_exposes_time_context() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(TIME_CONTEXT_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(TIME_CONTEXT_RULE, time_context_rule);
        let mut world = World::new();
        let mut trace = RuntimeTrace::new();
        let time_context = crate::runtime_time::TimeContext {
            frame_count: 3,
            delta_time: 0.25,
            unscaled_delta_time: 0.5,
            ..Default::default()
        };

        let results = runner.run_frame_update_with_time_context(
            3,
            &mut world,
            &mut trace,
            None,
            Some(time_context),
        );

        assert_eq!(results[0].writes[0].after, Some("0.25".to_string()));
    }

    #[test]
    fn logic_context_delta_time_matches_time_context() {
        let mut world = World::new();
        let world_api = WorldWriteApi::new(&mut world);
        let context = LogicContext::with_time_context(
            1,
            crate::runtime_time::TimeContext {
                delta_time: 0.125,
                ..Default::default()
            },
            RulePhase::FrameUpdate,
            world_api,
        );

        assert_eq!(context.delta_time, context.time().delta_time);
        assert_eq!(context.delta_time, 0.125);
    }

    #[test]
    fn project_logic_runner_reports_prefab_command_missing_runtime_context() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.instantiate_prefab")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule("project.instantiate_prefab", instantiate_prefab_rule);
        let mut world = World::new();
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);

        let apply = trace
            .gameplay_records
            .iter()
            .find(|record| record.operation == "command_apply")
            .expect("command apply trace");
        assert_eq!(apply.result, "failed");
        assert_eq!(apply.error_code.as_deref(), Some("missing_runtime_context"));
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn project_logic_runner_applies_prefab_instantiate_with_runtime_context() {
        let package = package_fixture();
        let mut instance_loader =
            crate::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
        instance_loader.asset_loader_mut().mount_bundle("startup");
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.instantiate_prefab")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule("project.instantiate_prefab", instantiate_prefab_rule);
        let mut world = World::new();
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update_with_runtime(
            1,
            &mut world,
            &mut trace,
            None,
            &package,
            &mut instance_loader,
        );

        assert_eq!(world.entity_count(), 2);
        assert!(instance_loader
            .prefab_instance(crate::runtime_instance::RuntimeInstanceId(1))
            .is_some());
        let apply = trace
            .gameplay_records
            .iter()
            .find(|record| record.operation == "command_apply")
            .expect("command apply trace");
        assert_eq!(apply.result, "ok");
        assert_eq!(apply.source.as_deref(), Some("prefab-ship"));
        assert!(apply
            .after
            .as_deref()
            .is_some_and(|summary| summary.contains("created_entity_count=2")));
    }

    #[test]
    fn project_logic_runner_applies_prefab_despawn_with_runtime_context() {
        let package = package_fixture();
        let mut instance_loader =
            crate::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
        instance_loader.asset_loader_mut().mount_bundle("startup");
        let mut world = World::new();
        let (instance, report) = instance_loader.instantiate_prefab_from_package(
            &package,
            asset_ref("prefab-ship", "prefab"),
            None,
            None,
            &mut world,
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        assert_eq!(
            instance.expect("prefab instance").instance_id,
            crate::runtime_instance::RuntimeInstanceId(1)
        );
        assert_eq!(world.entity_count(), 2);

        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot("project.despawn_prefab")],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule("project.despawn_prefab", despawn_prefab_instance_rule);
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update_with_runtime(
            2,
            &mut world,
            &mut trace,
            None,
            &package,
            &mut instance_loader,
        );

        assert_eq!(world.entity_count(), 0);
        assert!(instance_loader
            .prefab_instance(crate::runtime_instance::RuntimeInstanceId(1))
            .is_none());
        let apply = trace
            .gameplay_records
            .iter()
            .find(|record| record.operation == "command_apply")
            .expect("command apply trace");
        assert_eq!(apply.result, "ok");
        assert!(apply
            .after
            .as_deref()
            .is_some_and(|summary| summary.contains("created_entity_count=2")));
    }

    fn package_fixture() -> crate::runtime_package::RuntimePackage {
        use crate::runtime_asset::{
            BundleRecord, RuntimeAssetDependencyRecord, RuntimeAssetIndex, RuntimePackageMountTable,
        };
        use crate::runtime_package::{
            RuntimeAssetManifest, RuntimeInputManifest, RuntimeInputMappingManifestEntry,
            RuntimeManifestAssetIndex, RuntimeManifestInputIndex, RuntimeManifestRuleIndex,
            RuntimePackageManifest, RuntimeProjectInfo, RuntimeRuleManifest, RuntimeScene,
            RuntimeSceneManifestEntry, RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION,
            RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION, RUNTIME_PACKAGE_MODE,
            RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
            RUNTIME_SCENE_SCHEMA_VERSION,
        };
        use engine_input::InputMappingAsset;
        use std::path::PathBuf;

        let assets = RuntimeAssetManifest {
            schema_version: RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            assets: vec![
                runtime_asset("scene-main", "scene", None),
                runtime_asset("prefab-ship", "prefab", Some(prefab_json())),
            ],
            runtime_asset_index: vec![
                record("scene-main", "scene"),
                record("prefab-ship", "prefab"),
            ],
            bundle_table: vec![BundleRecord {
                bundle_id: "startup".to_string(),
                mount_id: None,
                uri: "bundles/startup".to_string(),
                hash: None,
                version: None,
                mounted: false,
            }],
            cooked_asset_table: vec![cooked("scene-main"), cooked("prefab-ship")],
            dependency_table: Vec::<RuntimeAssetDependencyRecord>::new(),
        };
        let runtime_asset_index = RuntimeAssetIndex::from_manifest(
            &assets,
            &assets.runtime_asset_index,
            &assets.cooked_asset_table,
            &assets.dependency_table,
        );
        let runtime_asset_mount_table = RuntimePackageMountTable::from_manifest(&assets);
        let default_input_mapping = InputMappingAsset::gameplay_default();
        let input_manifest = RuntimeInputManifest {
            schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.to_string(),
            default_mapping_id: default_input_mapping.asset_id.clone(),
            mappings: vec![RuntimeInputMappingManifestEntry {
                id: default_input_mapping.asset_id.clone(),
                path: "input/input.default.json".to_string(),
                enabled: true,
            }],
        };
        crate::runtime_package::RuntimePackage {
            package_dir: PathBuf::new(),
            manifest: RuntimePackageManifest {
                schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.to_string(),
                package_mode: RUNTIME_PACKAGE_MODE.to_string(),
                project: RuntimeProjectInfo::explicit_empty("project-fixture", "Fixture", "0.0.3"),
                active_scene_id: "scene-main".to_string(),
                scenes: vec![RuntimeSceneManifestEntry {
                    id: "scene-main".to_string(),
                    name: "Main".to_string(),
                    path: "scenes/scene-main.json".to_string(),
                    entity_count: 0,
                }],
                assets: RuntimeManifestAssetIndex {
                    path: "assets/asset-manifest.json".to_string(),
                    asset_count: 2,
                },
                rules: RuntimeManifestRuleIndex {
                    path: "rules/rule-manifest.json".to_string(),
                    mode: "none".to_string(),
                },
                input: RuntimeManifestInputIndex {
                    path: "input/input-manifest.json".to_string(),
                    default_mapping_id: default_input_mapping.asset_id.clone(),
                    mapping_count: 1,
                },
                aui: Some(crate::runtime_package::RuntimeManifestAuiIndex {
                    path: "aui/aui-manifest.json".to_string(),
                    document_count: 0,
                }),
                font_atlases: None,
                font_bundles: None,
                animator2d: None,
                observation_contract: None,
                content_hash: None,
            },
            active_scene: RuntimeScene {
                schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
                id: "scene-main".to_string(),
                name: "Main".to_string(),
                gravity: 0.0,
                background: "#000".to_string(),
                sky_color: "#111".to_string(),
                entities: Vec::new(),
            },
            assets,
            runtime_asset_index,
            runtime_asset_mount_table,
            rules: RuntimeRuleManifest {
                schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
                mode: "none".to_string(),
                rules: Vec::new(),
                modules: Vec::new(),
            },
            aui_manifest: crate::runtime_package::RuntimeAuiManifest::empty(),
            aui_documents: crate::runtime_package::RuntimeAuiDocumentRegistry::empty("fixture"),
            font_atlas_manifest: crate::runtime_package::RuntimeFontAtlasManifest::empty(),
            font_atlases: crate::runtime_package::RuntimeAuiFontAtlasRegistry::empty("fixture"),
            font_bundle_manifest: crate::font_bundle::RuntimeFontBundleManifest::empty(),
            font_bundles: crate::font_bundle::RuntimeFontBundleRegistry::default(),
            animator2d_registry: crate::animator2d::CookedAnimator2DRegistry::empty(),
            input_manifest,
            input_mappings: vec![default_input_mapping.clone()],
            default_input_mapping: Some(default_input_mapping),
        }
    }

    fn runtime_asset(
        id: &str,
        asset_type: &str,
        data: Option<serde_json::Value>,
    ) -> crate::runtime_package::RuntimeAsset {
        crate::runtime_package::RuntimeAsset {
            id: id.to_string(),
            name: id.to_string(),
            asset_type: asset_type.to_string(),
            source: format!("{}.asset", id),
            state: "available".to_string(),
            bundle_id: "startup".to_string(),
            data,
        }
    }

    fn record(id: &str, asset_type: &str) -> crate::runtime_asset::RuntimeAssetRecord {
        crate::runtime_asset::RuntimeAssetRecord {
            asset_guid: id.to_string(),
            asset_id: id.to_string(),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: "1".to_string(),
            cooked_asset_id: format!("cooked-{}", id),
            bundle_id: "startup".to_string(),
            loader_kind: asset_type.to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: Some(8),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn cooked(id: &str) -> crate::runtime_asset::CookedAssetRecord {
        crate::runtime_asset::CookedAssetRecord {
            cooked_asset_id: format!("cooked-{}", id),
            bundle_id: "startup".to_string(),
            path: None,
            offset: None,
            size: Some(8),
            compression: None,
            hash: None,
        }
    }

    fn prefab_json() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": "runtime-prefab.v1",
            "id": "prefab-ship",
            "name": "Ship",
            "rootEntityId": "ship-root",
            "entities": [
                prefab_entity_json("ship-root", serde_json::Value::Null),
                prefab_entity_json("ship-child", serde_json::json!("ship-root"))
            ]
        })
    }

    fn prefab_entity_json(id: &str, parent_id: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": "runtime-entity.v1",
            "id": id,
            "name": id,
            "kind": "actor",
            "enabled": true,
            "parentId": parent_id,
            "siblingOrder": 0,
            "transform": {
                "localPosition": { "x": 0.0, "y": 0.0, "z": 0.0 },
                "localRotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
                "localScale": { "x": 1.0, "y": 1.0, "z": 1.0 }
            },
            "components": []
        })
    }

    fn asset_ref(id: &str, asset_type: &str) -> crate::runtime_package::RuntimeAssetRef {
        crate::runtime_package::RuntimeAssetRef {
            id: id.to_string(),
            asset_type: asset_type.to_string(),
            guid: None,
            sub_asset: None,
        }
    }
}
