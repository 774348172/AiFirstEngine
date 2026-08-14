use crate::animator2d::{
    Animator2DCommand, Animator2DFrameResult, Animator2DModule, Animator2DReportLevel,
    CookedAnimator2DRegistry,
};
use crate::aui::AuiAction;
use crate::frame_hash::hash_frame;
use crate::input_action::{ActionSnapshot, InputTraceSummary};
use crate::physics2d::{
    CollisionPairReport, Physics2DBridge, Physics2DTraceRecord, Physics2DWorld,
};
use crate::project_logic::ProjectLogicRunner;
use crate::project_observation::CookedProjectObservationContract;
use crate::project_observation::ProjectRuntimeObservationState;
use crate::project_runtime_session::{
    execute_project_runtime_observation, execute_project_runtime_session_stage_with_animator2d,
    ProjectRuntimeSession, ProjectRuntimeSessionFrameReport, ProjectRuntimeSessionReportLevel,
    ProjectRuntimeSessionStage,
};
use crate::render_command::{apply_batch, RenderFrameReport, RenderFrameReportLevel};
use crate::render_extract::RenderExtractContext;
use crate::render_snapshot::{extract_render_snapshot, RenderSnapshot};
use crate::render_state::RenderSceneState;
use crate::runtime_instance_loader::RuntimeInstanceLoader;
use crate::runtime_package::RuntimePackage;
use crate::runtime_time::{RuntimeTime, TimeTraceSummary, DEFAULT_FIXED_DELTA_TIME};
use crate::runtime_trace::RuntimeTrace;
use crate::world::World;

#[derive(Debug, Clone, PartialEq)]
pub struct FrameOutput {
    pub frame: u64,
    pub snapshot: RenderSnapshot,
    pub frame_hash: String,
    pub trace: RuntimeTrace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeFrameOutput {
    pub frame_index: u64,
    pub frame_hash: String,
    pub runtime_trace: RuntimeTrace,
    pub render_frame_report: RenderFrameReport,
    pub merged_render_command_count: usize,
    pub applied_render_command_count: usize,
    pub render_scene_proxy_count: usize,
    pub physics2d_pair_report: CollisionPairReport,
    pub time_trace_summary: TimeTraceSummary,
    pub snapshot_compat: Option<RenderSnapshot>,
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    pub project_observation_state: Option<ProjectRuntimeObservationState>,
    pub animator2d_frame_result: Animator2DFrameResult,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeFrameSessionFault {
    pub frame_index: u64,
    pub runtime_trace: RuntimeTrace,
    pub time_trace_summary: TimeTraceSummary,
    pub report: ProjectRuntimeSessionFrameReport,
}

pub struct RuntimeFrameContext<'a> {
    pub package: &'a RuntimePackage,
    pub instance_loader: &'a mut RuntimeInstanceLoader,
}

pub struct ProjectRuntimeFrameSession<'a> {
    pub session: &'a mut dyn ProjectRuntimeSession,
    pub actions: &'a [AuiAction],
    pub report_level: ProjectRuntimeSessionReportLevel,
    pub observation_contract: Option<&'a CookedProjectObservationContract>,
}

#[derive(Debug, Clone)]
pub struct FrameLoop {
    scene_id: String,
    frame: u64,
    project_logic: ProjectLogicRunner,
    runtime_time: RuntimeTime,
    animator2d: Option<Animator2DModule>,
}

impl FrameLoop {
    pub fn new(scene_id: impl Into<String>) -> Self {
        Self {
            scene_id: scene_id.into(),
            frame: 0,
            project_logic: ProjectLogicRunner::empty(),
            runtime_time: RuntimeTime::new(),
            animator2d: None,
        }
    }

    pub fn with_project_logic(
        scene_id: impl Into<String>,
        project_logic: ProjectLogicRunner,
    ) -> Self {
        Self {
            scene_id: scene_id.into(),
            frame: 0,
            project_logic,
            runtime_time: RuntimeTime::new(),
            animator2d: None,
        }
    }

    pub fn project_logic(&self) -> &ProjectLogicRunner {
        &self.project_logic
    }

    pub fn project_logic_mut(&mut self) -> &mut ProjectLogicRunner {
        &mut self.project_logic
    }

    pub fn runtime_time(&self) -> &RuntimeTime {
        &self.runtime_time
    }

    pub fn runtime_time_mut(&mut self) -> &mut RuntimeTime {
        &mut self.runtime_time
    }

    pub fn set_animator2d_registry(
        &mut self,
        registry: CookedAnimator2DRegistry,
    ) -> Result<(), Vec<crate::animator2d::Animator2DDiagnostic>> {
        match self.animator2d.as_mut() {
            Some(module) => module.replace_registry(registry),
            None => {
                self.animator2d = Some(Animator2DModule::load(registry)?);
                Ok(())
            }
        }
    }

    pub fn animator2d_module(&self) -> Option<&Animator2DModule> {
        self.animator2d.as_ref()
    }

    pub(crate) fn apply_animator2d_commands(
        &mut self,
        commands: impl IntoIterator<Item = Animator2DCommand>,
    ) -> usize {
        let commands = commands.into_iter().collect::<Vec<_>>();
        let command_count = commands.len();
        if let Some(module) = self.animator2d.as_mut() {
            module.apply(commands);
        }
        command_count
    }

    pub fn tick(&mut self, world: &World) -> FrameOutput {
        self.frame += 1;
        let frame = self.frame;
        let mut trace = RuntimeTrace::new();
        trace.record(
            frame,
            "engine.frame_loop",
            "FrameBegin",
            "begin",
            Some(world.entity_count()),
        );
        trace.record(
            frame,
            "engine.frame_loop",
            "FixedUpdate",
            "fixed_update",
            Some(world.entity_count()),
        );
        trace.record(
            frame,
            "engine.frame_loop",
            "Update",
            "update",
            Some(world.entity_count()),
        );
        trace.record(
            frame,
            "engine.frame_loop",
            "LateUpdate",
            "late_update",
            Some(world.entity_count()),
        );
        let snapshot = extract_render_snapshot(self.scene_id.clone(), frame, world);
        trace.record(
            frame,
            "engine.render_extract",
            "RenderExtract",
            "end",
            Some(snapshot.renderables.len()),
        );
        let frame_hash = hash_frame(&self.scene_id, frame, world, &snapshot);
        trace.record(
            frame,
            "engine.frame_loop",
            "FrameEnd",
            "end",
            Some(world.entity_count()),
        );
        FrameOutput {
            frame,
            snapshot,
            frame_hash,
            trace,
        }
    }

    pub fn tick_runtime_frame(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
    ) -> RuntimeFrameOutput {
        self.tick_runtime_frame_with_input(world, render_scene, extract, None, None)
    }

    pub fn tick_runtime_frame_with_input(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
    ) -> RuntimeFrameOutput {
        self.tick_runtime_frame_with_input_and_delta(
            world,
            render_scene,
            extract,
            action_snapshot,
            input_trace_summary,
            DEFAULT_FIXED_DELTA_TIME,
        )
    }

    pub fn tick_runtime_frame_with_input_and_delta(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
        unscaled_delta_time: f32,
    ) -> RuntimeFrameOutput {
        self.tick_runtime_frame_with_input_delta_and_runtime(
            world,
            render_scene,
            extract,
            action_snapshot,
            input_trace_summary,
            unscaled_delta_time,
            None,
        )
    }

    pub fn tick_runtime_frame_with_runtime_context_and_delta(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
        unscaled_delta_time: f32,
        runtime_context: RuntimeFrameContext<'_>,
    ) -> RuntimeFrameOutput {
        self.tick_runtime_frame_with_input_delta_and_runtime(
            world,
            render_scene,
            extract,
            action_snapshot,
            input_trace_summary,
            unscaled_delta_time,
            Some(runtime_context),
        )
    }

    fn tick_runtime_frame_with_input_delta_and_runtime(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
        unscaled_delta_time: f32,
        runtime_context: Option<RuntimeFrameContext<'_>>,
    ) -> RuntimeFrameOutput {
        self.tick_runtime_frame_with_input_delta_runtime_and_session(
            world,
            render_scene,
            extract,
            action_snapshot,
            input_trace_summary,
            unscaled_delta_time,
            runtime_context,
            None,
        )
        .expect("frame execution without a project runtime session cannot session-fault")
    }

    pub fn tick_runtime_frame_with_project_session_and_delta(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
        unscaled_delta_time: f32,
        runtime_context: Option<RuntimeFrameContext<'_>>,
        project_session: ProjectRuntimeFrameSession<'_>,
    ) -> Result<RuntimeFrameOutput, RuntimeFrameSessionFault> {
        self.tick_runtime_frame_with_input_delta_runtime_and_session(
            world,
            render_scene,
            extract,
            action_snapshot,
            input_trace_summary,
            unscaled_delta_time,
            runtime_context,
            Some(project_session),
        )
    }

    fn tick_runtime_frame_with_input_delta_runtime_and_session(
        &mut self,
        world: &mut World,
        render_scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        action_snapshot: Option<&ActionSnapshot>,
        input_trace_summary: Option<InputTraceSummary>,
        unscaled_delta_time: f32,
        mut runtime_context: Option<RuntimeFrameContext<'_>>,
        mut project_session: Option<ProjectRuntimeFrameSession<'_>>,
    ) -> Result<RuntimeFrameOutput, RuntimeFrameSessionFault> {
        self.frame += 1;
        let frame = self.frame;
        let time_trace_summary = self.runtime_time.advance_frame(unscaled_delta_time);
        let frame_time_context = self.runtime_time.context();
        let mut trace = RuntimeTrace::new();
        trace.record(
            frame,
            "engine.frame_loop",
            "FrameBegin",
            "begin",
            Some(world.entity_count()),
        );
        let mut project_runtime_session_report = project_session.as_ref().map(|binding| {
            ProjectRuntimeSessionFrameReport::new(frame, binding.session.session_id().to_string())
        });
        let mut animator2d_commands = Vec::<Animator2DCommand>::new();
        if let Some(binding) = project_session.as_mut() {
            if !binding.actions.is_empty() {
                let stage_report = execute_project_runtime_session_stage_with_animator2d(
                    binding.session,
                    ProjectRuntimeSessionStage::AuiActionDispatch,
                    frame,
                    frame_time_context,
                    world,
                    binding.actions,
                    binding.report_level,
                    &mut animator2d_commands,
                );
                trace.record(
                    frame,
                    "project.runtime_session",
                    ProjectRuntimeSessionStage::AuiActionDispatch.as_str(),
                    format!("{:?}", stage_report.status),
                    Some(world.entity_count()),
                );
                let terminal_fault = stage_report.terminal_fault;
                project_runtime_session_report
                    .as_mut()
                    .expect("session report exists with session binding")
                    .push_stage(stage_report);
                if terminal_fault {
                    trace.record(
                        frame,
                        "engine.frame_loop",
                        "FrameFault",
                        "project_runtime_session",
                        Some(world.entity_count()),
                    );
                    return Err(RuntimeFrameSessionFault {
                        frame_index: frame,
                        runtime_trace: trace,
                        time_trace_summary,
                        report: project_runtime_session_report
                            .expect("session report exists on fault"),
                    });
                }
            }
        }
        let input_summary = input_trace_summary
            .unwrap_or_else(|| InputTraceSummary::from_snapshot(action_snapshot));
        if action_snapshot.is_some()
            || input_summary.action_count > 0
            || input_summary.route_kind.is_some()
        {
            trace.record_input_summary(&input_summary);
        }
        trace.record(
            frame,
            "engine.frame_loop",
            "FixedUpdate",
            "fixed_update",
            Some(world.entity_count()),
        );
        self.runtime_time.advance_fixed_step();
        let fixed_time_context = self.runtime_time.context();
        if let Some(binding) = project_session.as_mut() {
            let stage_report = execute_project_runtime_session_stage_with_animator2d(
                binding.session,
                ProjectRuntimeSessionStage::FixedUpdate,
                frame,
                fixed_time_context,
                world,
                &[],
                binding.report_level,
                &mut animator2d_commands,
            );
            trace.record(
                frame,
                "project.runtime_session",
                ProjectRuntimeSessionStage::FixedUpdate.as_str(),
                format!("{:?}", stage_report.status),
                Some(world.entity_count()),
            );
            let terminal_fault = stage_report.terminal_fault;
            project_runtime_session_report
                .as_mut()
                .expect("session report exists with session binding")
                .push_stage(stage_report);
            if terminal_fault {
                self.runtime_time.leave_fixed_step();
                trace.record(
                    frame,
                    "engine.frame_loop",
                    "FrameFault",
                    "project_runtime_session",
                    Some(world.entity_count()),
                );
                return Err(RuntimeFrameSessionFault {
                    frame_index: frame,
                    runtime_trace: trace,
                    time_trace_summary,
                    report: project_runtime_session_report.expect("session report exists on fault"),
                });
            }
        }
        if let Some(context) = runtime_context.as_mut() {
            self.project_logic
                .run_fixed_update_with_runtime_and_time_context(
                    frame,
                    world,
                    &mut trace,
                    action_snapshot,
                    Some(fixed_time_context),
                    context.package,
                    &mut *context.instance_loader,
                );
        } else {
            self.project_logic.run_fixed_update_with_time_context(
                frame,
                world,
                &mut trace,
                action_snapshot,
                Some(fixed_time_context),
            );
        }
        let project_observation_state = project_session.as_ref().and_then(|binding| {
            binding.observation_contract.map(|contract| {
                execute_project_runtime_observation(
                    &*binding.session,
                    frame,
                    fixed_time_context,
                    world,
                    contract,
                    binding.report_level,
                )
            })
        });
        if let Some(state) = &project_observation_state {
            project_runtime_session_report
                .as_mut()
                .expect("session report exists with observation state")
                .set_observation(state);
            trace.record(
                frame,
                "project.runtime_session",
                "RuntimeFramePostCommit",
                match state {
                    ProjectRuntimeObservationState::Published { .. } => "published",
                    ProjectRuntimeObservationState::ContractViolated { .. } => "contract_violated",
                    ProjectRuntimeObservationState::NotProducedYet { .. } => "not_produced_yet",
                },
                state.runtime_frame().map(|_| world.entity_count()),
            );
        }
        self.runtime_time.leave_fixed_step();
        trace.record(
            frame,
            "engine.frame_loop",
            "Update",
            "update",
            Some(world.entity_count()),
        );
        if let Some(context) = runtime_context.as_mut() {
            self.project_logic
                .run_frame_update_with_runtime_and_time_context(
                    frame,
                    world,
                    &mut trace,
                    action_snapshot,
                    Some(frame_time_context),
                    context.package,
                    &mut *context.instance_loader,
                );
        } else {
            self.project_logic.run_frame_update_with_time_context(
                frame,
                world,
                &mut trace,
                action_snapshot,
                Some(frame_time_context),
            );
        }
        let mut physics2d_world = Physics2DWorld::new();
        let physics2d_sync_report = Physics2DBridge::sync_from_world(world, &mut physics2d_world);
        trace.record_physics2d(Physics2DTraceRecord::sync(
            frame,
            "Physics2D",
            &physics2d_sync_report,
        ));
        trace.record(
            frame,
            "engine.physics2d",
            "Physics2D",
            "sync_from_world",
            Some(physics2d_sync_report.synced_colliders),
        );
        let physics2d_pair_report = physics2d_world.build_collision_pairs();
        trace.record_physics2d(Physics2DTraceRecord::pair_report(
            frame,
            "Physics2D",
            &physics2d_pair_report,
        ));
        trace.record(
            frame,
            "engine.physics2d",
            "Physics2D",
            "build_collision_pairs",
            Some(physics2d_pair_report.pairs.len()),
        );
        if let Some(context) = runtime_context.as_mut() {
            self.project_logic
                .run_post_physics_with_runtime_and_time_context(
                    frame,
                    world,
                    &mut trace,
                    action_snapshot,
                    Some(frame_time_context),
                    &physics2d_pair_report.pairs,
                    context.package,
                    &mut *context.instance_loader,
                );
        }
        if let Some(context) = runtime_context.as_ref() {
            self.set_animator2d_registry(context.package.animator2d_registry.clone())
                .expect("RuntimePackage Animator2D registry was validated during load");
        }
        let animator_report_level = project_session
            .as_ref()
            .map(|binding| match binding.report_level {
                ProjectRuntimeSessionReportLevel::Off => Animator2DReportLevel::Off,
                ProjectRuntimeSessionReportLevel::Summary => Animator2DReportLevel::Summary,
                ProjectRuntimeSessionReportLevel::Trace => Animator2DReportLevel::Trace,
            })
            .unwrap_or(Animator2DReportLevel::Summary);
        let animator2d_frame_result = if let Some(module) = self.animator2d.as_mut() {
            module.apply(animator2d_commands);
            module.tick(
                world,
                fixed_time_context.fixed_frame_count,
                animator_report_level,
            )
        } else {
            Animator2DFrameResult {
                fixed_tick_index: fixed_time_context.fixed_frame_count,
                failed_entity_count: animator2d_commands.len(),
                ..Animator2DFrameResult::default()
            }
        };
        trace.record(
            frame,
            "engine.animator2d",
            "Animator2D",
            "fixed_tick",
            Some(animator2d_frame_result.changed_entity_count),
        );
        trace.record(
            frame,
            "engine.frame_loop",
            "LateUpdate",
            "late_update",
            Some(world.entity_count()),
        );

        let mut queue = extract.extract_world_dirty(frame, world, render_scene);
        let merged_commands = queue.normalize_merge(render_scene);
        let apply_diagnostics = apply_batch(render_scene, &merged_commands);
        let render_frame_report = queue.build_report(
            RenderFrameReportLevel::Summary,
            &merged_commands,
            &apply_diagnostics,
        );
        trace.record(
            frame,
            "engine.render_extract",
            "RenderExtract",
            "commands_applied",
            Some(merged_commands.len()),
        );

        let snapshot_compat = extract_render_snapshot(self.scene_id.clone(), frame, world);
        let frame_hash = hash_frame(&self.scene_id, frame, world, &snapshot_compat);
        trace.record(
            frame,
            "engine.frame_loop",
            "FrameEnd",
            "end",
            Some(world.entity_count()),
        );
        let time_trace_summary = self.runtime_time.trace_summary();

        Ok(RuntimeFrameOutput {
            frame_index: frame,
            frame_hash,
            runtime_trace: trace,
            render_frame_report,
            merged_render_command_count: merged_commands.len(),
            applied_render_command_count: merged_commands
                .len()
                .saturating_sub(apply_diagnostics.len()),
            render_scene_proxy_count: render_scene.proxies_len(),
            physics2d_pair_report,
            time_trace_summary,
            snapshot_compat: Some(snapshot_compat),
            project_runtime_session_report,
            project_observation_state,
            animator2d_frame_result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animator2d::{
        Animator2DParameterKind, Animator2DPlayback, Animator2DTransitionTiming,
        CookedAnimator2DCondition, CookedAnimator2DParameter, CookedAnimator2DState,
        CookedAnimator2DTransition, CookedAnimatorController2D, CookedSpriteAnimationClip2D,
        CookedSpriteAnimationFrame2D, RuntimeAnimator2D,
    };
    use crate::archetype::ComponentValue;
    use crate::components::{ComponentTypeId, Hierarchy, SpriteRenderer2D, Transform};
    use crate::ids::EntityId;
    use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
    use crate::math::{Vec2, Vec3};
    use crate::physics2d::Collider2D;
    use crate::project_logic::{ProjectLogicRunner, RuleCall, RuleExecutionPlan};
    use crate::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeMutationBuffer, ProjectRuntimeSessionContext,
        ProjectRuntimeSessionOutput,
    };
    use crate::scene_loader::load_scene_into_world;
    use crate::scene_loader::tests_support::renderable_scene_fixture;

    const FRAME_MOVE_RULE: &str = "project.frame_move";
    const FIRE_MOVE_RULE: &str = "project.fire_move";
    const PHYSICS_MOVE_RULE: &str = "project.physics_move";
    const TIME_DELTA_RULE: &str = "project.time_delta";

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn transform(x: f32, y: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn frame_move_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = crate::ids::EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform should exist");
        position.x += 5.0;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write should succeed");
        let mut result = LogicResult::applied(FRAME_MOVE_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn fire_move_rule(context: &mut LogicContext<'_>) -> LogicResult {
        if !context.action_pressed("action.fire") {
            return LogicResult::skipped(FIRE_MOVE_RULE, ExecutorKind::RustAot);
        }
        let entity_id = crate::ids::EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform should exist");
        position.x += 1.0;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write should succeed");
        let mut result = LogicResult::applied(FIRE_MOVE_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn physics_move_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = EntityId::from("entity-source");
        let write = context
            .write_transform_local_position(
                entity_id,
                Vec3 {
                    x: 1.5,
                    y: 0.0,
                    z: 0.0,
                },
            )
            .expect("write should succeed");
        let mut result = LogicResult::applied(PHYSICS_MOVE_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn time_delta_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = crate::ids::EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform should exist");
        position.x += context.time().delta_time;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write should succeed");
        let mut result = LogicResult::applied(TIME_DELTA_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn frame_move_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(FRAME_MOVE_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(FRAME_MOVE_RULE, frame_move_rule);
        runner
    }

    fn fire_move_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(FIRE_MOVE_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(FIRE_MOVE_RULE, fire_move_rule);
        runner
    }

    fn physics_move_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(PHYSICS_MOVE_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(PHYSICS_MOVE_RULE, physics_move_rule);
        runner
    }

    fn time_delta_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: Vec::new(),
            frame_update: vec![RuleCall::rust_aot(TIME_DELTA_RULE)],
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(TIME_DELTA_RULE, time_delta_rule);
        runner
    }

    fn physics_pair_world() -> World {
        let mut world = World::new();
        for (entity_id, x) in [("entity-source", 0.0), ("entity-target", 2.0)] {
            let id = EntityId::from(entity_id);
            world.spawn_with_components(
                id.clone(),
                entity_id,
                "actor",
                true,
                hierarchy(),
                Some(transform(x, 0.0)),
                None,
            );
            world.insert_component_value(
                id,
                ComponentValue::Collider2D(Collider2D::aabb(Vec2 { x: 0.5, y: 0.5 })),
            );
        }
        world.take_dirty_records();
        world
    }

    #[test]
    fn render_extract_maps_mesh_asset_ref_to_mesh_ref() {
        let scene = renderable_scene_fixture();
        let world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut frame_loop = FrameLoop::new(scene.id);
        let output = frame_loop.tick(&world);
        assert_eq!(output.snapshot.renderables.len(), 1);
        assert_eq!(
            output.snapshot.renderables[0].mesh_ref.as_deref(),
            Some("model-player")
        );
    }

    #[test]
    fn fixed_tick_produces_stable_frame_hash() {
        let scene_a = renderable_scene_fixture();
        let scene_b = renderable_scene_fixture();
        let world_a = load_scene_into_world(&scene_a)
            .value
            .expect("world a should load");
        let world_b = load_scene_into_world(&scene_b)
            .value
            .expect("world b should load");
        let mut loop_a = FrameLoop::new(scene_a.id);
        let mut loop_b = FrameLoop::new(scene_b.id);
        let hash_a = loop_a.tick(&world_a).frame_hash;
        let hash_b = loop_b.tick(&world_b).frame_hash;
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn runtime_trace_records_frame_phases() {
        let scene = renderable_scene_fixture();
        let world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut frame_loop = FrameLoop::new(scene.id);
        let output = frame_loop.tick(&world);
        let phases = output
            .trace
            .events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>();
        assert!(phases.contains(&"FrameBegin"));
        assert!(phases.contains(&"FixedUpdate"));
        assert!(phases.contains(&"RenderExtract"));
        assert!(phases.contains(&"FrameEnd"));
    }

    #[test]
    fn runtime_frame_consumes_dirty_records_once() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let entity_id = crate::ids::EntityId::from("entity-player");
        world.insert_transform(entity_id, crate::components::Transform::identity());
        let mut frame_loop = FrameLoop::new(scene.id);
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let dirty_before = world.dirty_records().len();

        let first = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);
        let second = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(
            first.render_frame_report.counters.raw_command_count,
            dirty_before
        );
        assert_eq!(second.render_frame_report.counters.raw_command_count, 0);
        assert!(world.dirty_records().is_empty());
    }

    #[test]
    fn runtime_frame_applies_render_commands_to_scene_state() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let entity_id = crate::ids::EntityId::from("entity-player");
        world.insert_renderable(
            entity_id,
            crate::components::Renderable {
                mesh_ref: Some("model-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            },
        );
        let mut frame_loop = FrameLoop::new(scene.id);
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(output.render_scene_proxy_count, 1);
        assert_eq!(render_scene.proxies_len(), 1);
        assert_eq!(output.render_frame_report.counters.applied_command_count, 1);
    }

    #[test]
    fn runtime_frame_runs_logic_before_render_extract() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        world.take_dirty_records();
        let mut frame_loop = FrameLoop::with_project_logic(scene.id, frame_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(
            world
                .transform(&crate::ids::EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            5.0
        );
        assert_eq!(output.render_frame_report.counters.raw_command_count, 1);
        assert_eq!(output.render_frame_report.counters.applied_command_count, 1);
        assert!(output
            .runtime_trace
            .events
            .iter()
            .any(|event| event.system_id == "project.rule.project.frame_move"));
    }

    #[test]
    fn empty_project_logic_plan_preserves_existing_frame_output() {
        let scene_a = renderable_scene_fixture();
        let scene_b = renderable_scene_fixture();
        let mut world_a = load_scene_into_world(&scene_a)
            .value
            .expect("world a should load");
        let mut world_b = load_scene_into_world(&scene_b)
            .value
            .expect("world b should load");
        world_a.insert_transform(
            crate::ids::EntityId::from("entity-player"),
            crate::components::Transform {
                local_position: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
        );
        world_b.insert_transform(
            crate::ids::EntityId::from("entity-player"),
            crate::components::Transform {
                local_position: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
        );
        let mut loop_a = FrameLoop::new(scene_a.id);
        let mut loop_b = FrameLoop::with_project_logic(scene_b.id, ProjectLogicRunner::empty());
        let mut render_scene_a = RenderSceneState::new();
        let mut render_scene_b = RenderSceneState::new();
        let mut extract_a = RenderExtractContext::new();
        let mut extract_b = RenderExtractContext::new();

        let output_a = loop_a.tick_runtime_frame(&mut world_a, &mut render_scene_a, &mut extract_a);
        let output_b = loop_b.tick_runtime_frame(&mut world_b, &mut render_scene_b, &mut extract_b);

        assert_eq!(output_a.frame_hash, output_b.frame_hash);
        assert_eq!(
            output_a.render_frame_report.counters.raw_command_count,
            output_b.render_frame_report.counters.raw_command_count
        );
    }

    #[test]
    fn runtime_frame_accepts_none_action_snapshot() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut frame_loop = FrameLoop::with_project_logic(scene.id, fire_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame_with_input(
            &mut world,
            &mut render_scene,
            &mut extract,
            None,
            None,
        );

        assert_eq!(
            world
                .transform(&crate::ids::EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            0.0
        );
        assert!(output
            .runtime_trace
            .events
            .iter()
            .any(|event| event.message.contains("RustAotExecutor skipped")));
    }

    #[test]
    fn runtime_frame_exposes_action_snapshot_to_project_logic() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        world.take_dirty_records();
        let snapshot = crate::input_action::ActionSnapshot::with_actions(
            1,
            vec![crate::input_action::InputActionState::button(
                "action.fire",
                crate::input_action::ActionPhase::Pressed,
            )],
        );
        let mut frame_loop = FrameLoop::with_project_logic(scene.id, fire_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame_with_input(
            &mut world,
            &mut render_scene,
            &mut extract,
            Some(&snapshot),
            Some(crate::input_action::InputTraceSummary::from_snapshot(Some(
                &snapshot,
            ))),
        );

        assert_eq!(
            world
                .transform(&crate::ids::EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            1.0
        );
        assert_eq!(output.render_frame_report.counters.raw_command_count, 1);
        assert!(output
            .runtime_trace
            .events
            .iter()
            .any(|event| event.phase == "InputSnapshotReady"
                && event.message.contains("action.fire")));
    }

    #[test]
    fn runtime_frame_hash_is_stable_for_same_input() {
        let scene_a = renderable_scene_fixture();
        let scene_b = renderable_scene_fixture();
        let mut world_a = load_scene_into_world(&scene_a)
            .value
            .expect("world a should load");
        let mut world_b = load_scene_into_world(&scene_b)
            .value
            .expect("world b should load");
        let mut loop_a = FrameLoop::new(scene_a.id);
        let mut loop_b = FrameLoop::new(scene_b.id);
        let mut render_scene_a = RenderSceneState::new();
        let mut render_scene_b = RenderSceneState::new();
        let mut extract_a = RenderExtractContext::new();
        let mut extract_b = RenderExtractContext::new();

        let hash_a = loop_a
            .tick_runtime_frame(&mut world_a, &mut render_scene_a, &mut extract_a)
            .frame_hash;
        let hash_b = loop_b
            .tick_runtime_frame(&mut world_b, &mut render_scene_b, &mut extract_b)
            .frame_hash;

        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn frame_loop_runs_physics2d_after_project_logic_before_render_extract() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::with_project_logic("scene-physics", physics_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        let project_event_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == "project.rule.project.physics_move")
            .expect("project rule event should exist");
        let physics_event_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| {
                event.system_id == "engine.physics2d" && event.message == "sync_from_world"
            })
            .expect("physics sync event should exist");
        let render_event_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == "engine.render_extract")
            .expect("render extract event should exist");
        assert!(project_event_index < physics_event_index);
        assert!(physics_event_index < render_event_index);
    }

    #[test]
    fn frame_loop_physics2d_pair_report_reflects_transform_write() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::with_project_logic("scene-physics", physics_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(output.physics2d_pair_report.pairs.len(), 1);
        assert_eq!(
            output.physics2d_pair_report.pairs[0].entity_a,
            EntityId::from("entity-source")
        );
        assert_eq!(
            output.physics2d_pair_report.pairs[0].entity_b,
            EntityId::from("entity-target")
        );
    }

    #[test]
    fn frame_loop_physics2d_trace_is_recorded() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::with_project_logic("scene-physics", physics_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert!(output
            .runtime_trace
            .physics2d_records
            .iter()
            .any(|record| record.operation == "sync_from_world"
                && record.hit_count == Some(2)
                && record.result == "ok"));
        assert!(output
            .runtime_trace
            .physics2d_records
            .iter()
            .any(|record| record.operation == "build_collision_pairs"
                && record.pair_count == Some(1)
                && record.result == "ok"));
    }

    #[test]
    fn frame_loop_physics2d_does_not_write_back_transform_in_c_min() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::with_project_logic("scene-physics", physics_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(
            world
                .transform(&EntityId::from("entity-source"))
                .unwrap()
                .local_position,
            Vec3 {
                x: 1.5,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(
            world
                .transform(&EntityId::from("entity-target"))
                .unwrap()
                .local_position,
            Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn physics2d_foundation_c_min_end_to_end_overlap_after_transform_write() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::new("scene-physics");
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let first = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);
        assert!(first.physics2d_pair_report.pairs.is_empty());

        *frame_loop.project_logic_mut() = physics_move_runner();

        let second = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(second.physics2d_pair_report.pairs.len(), 1);
    }

    #[test]
    fn physics2d_foundation_c_min_query_api_supports_project_rule_usage() {
        let mut physics_world = crate::physics2d::Physics2DWorld::new();
        let source = EntityId::from("entity-source");
        let target = EntityId::from("entity-target");
        let collider = Collider2D::aabb(Vec2 { x: 0.5, y: 0.5 });
        for (entity_id, x) in [(source, 1.5), (target, 2.0)] {
            physics_world.insert_or_update_collider(
                crate::physics2d::Physics2DColliderProxy::from_transform_and_collider(
                    entity_id,
                    &transform(x, 0.0),
                    &collider,
                ),
            );
        }

        let hits = physics_world.overlap_aabb(&crate::physics2d::OverlapAabb2D::new(
            Vec2 { x: 1.5, y: 0.0 },
            Vec2 { x: 0.5, y: 0.5 },
        ));

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].entity_id, EntityId::from("entity-source"));
        assert_eq!(hits[1].entity_id, EntityId::from("entity-target"));
    }

    #[test]
    fn physics2d_foundation_c_min_no_project_semantic_terms_in_trace() {
        let mut world = physics_pair_world();
        let mut frame_loop = FrameLoop::with_project_logic("scene-physics", physics_move_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);
        let trace_text = format!("{:?}", output.runtime_trace.physics2d_records).to_lowercase();

        for forbidden in ["bullet", "enemy", "damage", "health", "score"] {
            assert!(
                !trace_text.contains(forbidden),
                "trace contains {forbidden}"
            );
        }
    }

    #[test]
    fn frame_loop_advances_runtime_time_once_per_frame() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut frame_loop = FrameLoop::with_project_logic(scene.id, time_delta_runner());
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame_with_input_and_delta(
            &mut world,
            &mut render_scene,
            &mut extract,
            None,
            None,
            0.25,
        );

        assert_eq!(output.time_trace_summary.frame_count, 1);
        assert_eq!(output.time_trace_summary.unscaled_delta_time, 0.25);
        assert_eq!(
            world
                .transform(&crate::ids::EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            0.25
        );
    }

    #[test]
    fn fixed_update_uses_fixed_delta_time() {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: vec![RuleCall::rust_aot(TIME_DELTA_RULE)],
            frame_update: Vec::new(),
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(TIME_DELTA_RULE, time_delta_rule);
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut frame_loop = FrameLoop::with_project_logic(scene.id, runner);
        frame_loop.runtime_time_mut().set_fixed_delta_time(0.02);
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        frame_loop.tick_runtime_frame_with_input_and_delta(
            &mut world,
            &mut render_scene,
            &mut extract,
            None,
            None,
            0.25,
        );

        assert_eq!(
            world
                .transform(&crate::ids::EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            0.02
        );
    }

    #[test]
    fn animator2d_schedule_same_tick_intent_runs_before_render_extract() {
        let registry = animator_schedule_registry();
        let mut world = animator_schedule_world(&registry.registry_digest);
        let mut frame_loop = FrameLoop::new("animator-scene");
        frame_loop.set_animator2d_registry(registry).unwrap();
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let mut session = AnimatorScheduleSession { emit_trigger: true };

        let output = frame_loop
            .tick_runtime_frame_with_project_session_and_delta(
                &mut world,
                &mut render_scene,
                &mut extract,
                None,
                None,
                DEFAULT_FIXED_DELTA_TIME,
                None,
                ProjectRuntimeFrameSession {
                    session: &mut session,
                    actions: &[],
                    report_level: ProjectRuntimeSessionReportLevel::Trace,
                    observation_contract: None,
                },
            )
            .unwrap();

        assert_eq!(
            world
                .sprite_renderer2d(&EntityId::from("animated"))
                .unwrap()
                .sprite_ref
                .as_deref(),
            Some("attack-0")
        );
        assert_eq!(output.animator2d_frame_result.transition_count, 1);
        let fixed = output
            .runtime_trace
            .events
            .iter()
            .position(|event| {
                event.system_id == "project.runtime_session" && event.phase == "fixed_update"
            })
            .unwrap();
        let animator = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == "engine.animator2d")
            .unwrap();
        let render = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == "engine.render_extract")
            .unwrap();
        assert!(fixed < animator && animator < render);

        session.emit_trigger = false;
        let paused_state = frame_loop
            .animator2d_module()
            .unwrap()
            .entity_state(&EntityId::from("animated"))
            .unwrap();
        assert_eq!(
            frame_loop
                .animator2d_module()
                .unwrap()
                .entity_state(&EntityId::from("animated"))
                .unwrap(),
            paused_state
        );
        let stepped = frame_loop
            .tick_runtime_frame_with_project_session_and_delta(
                &mut world,
                &mut render_scene,
                &mut extract,
                None,
                None,
                DEFAULT_FIXED_DELTA_TIME,
                None,
                ProjectRuntimeFrameSession {
                    session: &mut session,
                    actions: &[],
                    report_level: ProjectRuntimeSessionReportLevel::Summary,
                    observation_contract: None,
                },
            )
            .unwrap();
        assert_eq!(stepped.animator2d_frame_result.fixed_tick_index, 2);
    }

    #[test]
    fn animator2d_projection_sprite_change_is_consumed_by_existing_render_extract() {
        let registry = animator_schedule_registry();
        let mut world = animator_schedule_world(&registry.registry_digest);
        let mut frame_loop = FrameLoop::new("animator-projection");
        frame_loop.set_animator2d_registry(registry).unwrap();
        let mut render_scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();

        let output = frame_loop.tick_runtime_frame(&mut world, &mut render_scene, &mut extract);

        assert_eq!(output.animator2d_frame_result.changed_entity_count, 1);
        assert!(output.render_frame_report.counters.raw_command_count > 0);
        assert_eq!(render_scene.proxies_len(), 1);
    }

    struct AnimatorScheduleSession {
        emit_trigger: bool,
    }

    impl ProjectRuntimeSession for AnimatorScheduleSession {
        fn session_id(&self) -> &str {
            "animator.schedule.session"
        }

        fn handle_aui_actions(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
            batch: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut output = ProjectRuntimeSessionOutput::no_op();
            output.unhandled_action_count = batch.len();
            output
        }

        fn fixed_update(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            if !self.emit_trigger {
                return ProjectRuntimeSessionOutput::no_op();
            }
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.animator2d_set_trigger(EntityId::from("animated"), "attack");
            ProjectRuntimeSessionOutput::applied(mutations)
        }
    }

    fn animator_schedule_world(registry_digest: &str) -> World {
        let mut world = World::new();
        let entity_id = EntityId::from("animated");
        world
            .try_spawn_entity(entity_id.clone(), "Animated", "actor", true, hierarchy())
            .unwrap();
        world
            .try_insert_transform(entity_id.clone(), transform(0.0, 0.0))
            .unwrap();
        world
            .try_insert_sprite_renderer2d(entity_id.clone(), SpriteRenderer2D::default())
            .unwrap();
        world
            .try_insert_component_value(
                entity_id,
                ComponentTypeId::animator2d(),
                ComponentValue::Animator2D(RuntimeAnimator2D {
                    controller_id: "controller".to_string(),
                    controller_index: 0,
                    registry_digest: registry_digest.to_string(),
                    enabled: true,
                    initial_bools: Default::default(),
                }),
            )
            .unwrap();
        world
    }

    fn animator_schedule_registry() -> CookedAnimator2DRegistry {
        CookedAnimator2DRegistry::from_parts(
            vec![
                CookedSpriteAnimationClip2D {
                    id: "attack".to_string(),
                    playback: Animator2DPlayback::Once,
                    frames: vec![CookedSpriteAnimationFrame2D {
                        sprite_asset_id: "attack-0".to_string(),
                        duration_ticks: 2,
                    }],
                },
                CookedSpriteAnimationClip2D {
                    id: "idle".to_string(),
                    playback: Animator2DPlayback::Loop,
                    frames: vec![CookedSpriteAnimationFrame2D {
                        sprite_asset_id: "idle-0".to_string(),
                        duration_ticks: 2,
                    }],
                },
            ],
            vec![CookedAnimatorController2D {
                id: "controller".to_string(),
                entry_state_index: 1,
                parameters: vec![CookedAnimator2DParameter {
                    id: "attack".to_string(),
                    kind: Animator2DParameterKind::Trigger,
                    default_bool: false,
                }],
                states: vec![
                    CookedAnimator2DState {
                        id: "attack".to_string(),
                        clip_index: 0,
                        speed_permille: 1000,
                    },
                    CookedAnimator2DState {
                        id: "idle".to_string(),
                        clip_index: 1,
                        speed_permille: 1000,
                    },
                ],
                transitions: vec![CookedAnimator2DTransition {
                    id: "idle-to-attack".to_string(),
                    from_state_index: 1,
                    to_state_index: 0,
                    timing: Animator2DTransitionTiming::Immediate,
                    priority: 100,
                    conditions: vec![CookedAnimator2DCondition::Triggered { parameter_index: 0 }],
                }],
            }],
        )
        .unwrap()
    }
}
