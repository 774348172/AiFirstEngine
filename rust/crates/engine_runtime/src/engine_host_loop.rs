use crate::animator2d::{Animator2DFrameResult, Animator2DModule};
use crate::archetype::ComponentValue;
use crate::aui::{AuiAction, AuiCompositionFrame, AuiInteractionResult, AuiOverlayFrame};
use crate::component_value::RuntimeValue;
use crate::components::ComponentTypeId;
use crate::frame_loop::{
    FrameLoop, ProjectRuntimeFrameSession, RuntimeFrameContext, RuntimeFrameOutput,
};
use crate::game_view_presentation::{GameViewTargetSpec, ResolvedGameViewPresentation};
use crate::input_action::{ActionSnapshot, InputTraceSummary};
use crate::minimal_renderer::{MinimalRenderer, MinimalRendererFrame};
use crate::project_observation::{
    CookedProjectObservationContract, ProjectRuntimeObservationState,
};
use crate::project_runtime_session::{
    execute_project_runtime_observation, execute_project_runtime_session_stage_with_animator2d,
    EmptyProjectRuntimeSession, ProjectRuntimeSession, ProjectRuntimeSessionFrameReport,
    ProjectRuntimeSessionReportLevel, ProjectRuntimeSessionStage,
};
use crate::render_command::RenderFrameReport;
use crate::render_extract::RenderExtractContext;
use crate::render_state::{
    RenderSceneState, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState, Viewport,
};
use crate::render_thread::{
    RenderFramePacket, RenderSubmissionReport, RenderThreadConfig, RenderThreadFrameOutput,
    RenderThreadMode,
};
use crate::render_thread_worker::{
    FrameLagController, RenderCommandDispatcher, RenderFenceSyncDepth, RenderWorkerReport,
};
use crate::renderer_feature_builder::{RendererFeatureBuilder, RendererFeatureFrame};
use crate::runtime_renderer::{QualityProfile, RenderTarget};
use crate::runtime_texture::RuntimeTextureBindingContext;
use crate::runtime_time::{TimeTraceSummary, DEFAULT_FIXED_DELTA_TIME};
use crate::runtime_trace::RuntimeTrace;
use crate::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use crate::world::World;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineHostMode {
    HeadlessServer,
    ExportedGame,
    EditorPlay,
    EditorStep,
    EditorPause,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineFrameInput {
    pub mode: EngineHostMode,
    pub action_snapshot: Option<ActionSnapshot>,
    pub input_trace_summary: Option<InputTraceSummary>,
    pub aui_overlay: Option<AuiOverlayFrame>,
    pub aui_composition: Option<AuiCompositionFrame>,
    pub aui_interaction: Option<AuiInteractionResult>,
    pub runtime_texture_bindings: Option<RuntimeTextureBindingContext>,
    pub unscaled_delta_time: f32,
    pub fixed_step_count: usize,
}

impl EngineFrameInput {
    pub fn new(mode: EngineHostMode) -> Self {
        Self {
            mode,
            action_snapshot: None,
            input_trace_summary: None,
            aui_overlay: None,
            aui_composition: None,
            aui_interaction: None,
            runtime_texture_bindings: None,
            unscaled_delta_time: DEFAULT_FIXED_DELTA_TIME,
            fixed_step_count: 1,
        }
    }

    pub fn with_action_snapshot(mut self, action_snapshot: ActionSnapshot) -> Self {
        self.action_snapshot = Some(action_snapshot);
        self
    }

    pub fn with_input_trace_summary(mut self, summary: InputTraceSummary) -> Self {
        self.input_trace_summary = Some(summary);
        self
    }

    pub fn with_aui_overlay(mut self, overlay: AuiOverlayFrame) -> Self {
        self.aui_overlay = Some(overlay);
        self
    }

    pub fn with_aui_composition(mut self, composition: AuiCompositionFrame) -> Self {
        self.aui_composition = Some(composition);
        self
    }

    pub fn with_aui_interaction(mut self, interaction: AuiInteractionResult) -> Self {
        self.aui_interaction = Some(interaction);
        self
    }

    pub fn with_runtime_texture_bindings(mut self, bindings: RuntimeTextureBindingContext) -> Self {
        self.runtime_texture_bindings = Some(bindings);
        self
    }

    pub fn with_unscaled_delta_time(mut self, unscaled_delta_time: f32) -> Self {
        self.unscaled_delta_time = unscaled_delta_time;
        self
    }

    pub fn with_fixed_step_count(mut self, fixed_step_count: usize) -> Self {
        self.fixed_step_count = fixed_step_count;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineFrameOutput {
    pub frame_index: u64,
    pub runtime_advanced: bool,
    pub render_built: bool,
    pub runtime_trace: RuntimeTrace,
    pub render_frame_report: Option<RenderFrameReport>,
    pub renderer_feature_frame: Option<RendererFeatureFrame>,
    pub minimal_renderer_frame: Option<MinimalRendererFrame>,
    pub render_thread_frame: Option<RenderThreadFrameOutput>,
    pub render_submission_report: Option<RenderSubmissionReport>,
    pub render_worker_report: Option<RenderWorkerReport>,
    pub frame_hash: Option<String>,
    pub time_trace_summary: Option<TimeTraceSummary>,
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    pub project_observation_state: Option<ProjectRuntimeObservationState>,
    pub animator2d_frame_result: Animator2DFrameResult,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImmediateAuiActionDispatchOutput {
    pub frame_index: u64,
    pub runtime_advanced: bool,
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    pub project_observation_state: Option<ProjectRuntimeObservationState>,
    pub animator2d_command_count: usize,
    pub terminal_fault: bool,
}

pub struct EngineHostLoop {
    frame_loop: FrameLoop,
    project_runtime_session: Box<dyn ProjectRuntimeSession>,
    project_runtime_session_report_level: ProjectRuntimeSessionReportLevel,
    project_runtime_session_faulted: bool,
    project_observation_contract: Option<CookedProjectObservationContract>,
    project_observation_state: Option<ProjectRuntimeObservationState>,
    render_scene: RenderSceneState,
    extract: RenderExtractContext,
    feature_builder: RendererFeatureBuilder,
    minimal_renderer: MinimalRenderer,
    render_dispatcher: RenderCommandDispatcher,
    frame_lag_controller: FrameLagController,
    game_view_target: GameViewTargetSpec,
    host_frame: u64,
}

impl fmt::Debug for EngineHostLoop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EngineHostLoop")
            .field("frame_loop", &self.frame_loop)
            .field(
                "project_runtime_session_id",
                &self.project_runtime_session.session_id(),
            )
            .field(
                "project_runtime_session_report_level",
                &self.project_runtime_session_report_level,
            )
            .field(
                "project_runtime_session_faulted",
                &self.project_runtime_session_faulted,
            )
            .field("render_scene", &self.render_scene)
            .field("extract", &self.extract)
            .field("host_frame", &self.host_frame)
            .finish_non_exhaustive()
    }
}

impl EngineHostLoop {
    pub fn new(scene_id: impl Into<String>) -> Self {
        Self {
            frame_loop: FrameLoop::new(scene_id),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::inline(RenderThreadConfig::default()),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    pub fn with_project_logic(
        scene_id: impl Into<String>,
        project_logic: crate::project_logic::ProjectLogicRunner,
    ) -> Self {
        Self {
            frame_loop: FrameLoop::with_project_logic(scene_id, project_logic),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::inline(RenderThreadConfig::default()),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    pub fn with_project_runtime_session(
        scene_id: impl Into<String>,
        project_logic: crate::project_logic::ProjectLogicRunner,
        project_runtime_session: Box<dyn ProjectRuntimeSession>,
    ) -> Self {
        Self {
            frame_loop: FrameLoop::with_project_logic(scene_id, project_logic),
            project_runtime_session,
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::inline(RenderThreadConfig::default()),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    pub fn set_project_runtime_session_report_level(
        &mut self,
        report_level: ProjectRuntimeSessionReportLevel,
    ) {
        self.project_runtime_session_report_level = report_level;
    }

    pub fn set_game_view_target(&mut self, target: GameViewTargetSpec) {
        self.game_view_target = target;
    }

    pub fn new_with_render_mode(scene_id: impl Into<String>, mode: RenderThreadMode) -> Self {
        Self {
            frame_loop: FrameLoop::new(scene_id),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::from_thread_mode(mode),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    pub fn render_scene(&self) -> &RenderSceneState {
        &self.render_scene
    }

    pub fn project_observation_state(&self) -> Option<&ProjectRuntimeObservationState> {
        self.project_observation_state.as_ref()
    }

    pub fn animator2d_module(&self) -> Option<&Animator2DModule> {
        self.frame_loop.animator2d_module()
    }

    pub fn dispatch_aui_actions_immediately(
        &mut self,
        actions: &[AuiAction],
        world: &mut World,
    ) -> ImmediateAuiActionDispatchOutput {
        let frame_index = self.frame_loop.runtime_time().frame_count;
        if self.project_runtime_session_faulted {
            let report = ProjectRuntimeSessionFrameReport::reentry_after_fault(
                frame_index,
                self.project_runtime_session.session_id().to_string(),
            );
            return ImmediateAuiActionDispatchOutput {
                frame_index,
                runtime_advanced: false,
                project_runtime_session_report: Some(report),
                project_observation_state: self.project_observation_state.clone(),
                animator2d_command_count: 0,
                terminal_fault: true,
            };
        }

        if actions.is_empty() {
            return ImmediateAuiActionDispatchOutput {
                frame_index,
                runtime_advanced: false,
                project_runtime_session_report: None,
                project_observation_state: self.project_observation_state.clone(),
                animator2d_command_count: 0,
                terminal_fault: false,
            };
        }

        let time = self.frame_loop.runtime_time().context();
        let mut animator2d_commands = Vec::new();
        let stage_report = execute_project_runtime_session_stage_with_animator2d(
            self.project_runtime_session.as_mut(),
            ProjectRuntimeSessionStage::AuiActionDispatch,
            frame_index,
            time,
            world,
            actions,
            self.project_runtime_session_report_level,
            &mut animator2d_commands,
        );
        let terminal_fault = stage_report.terminal_fault;
        let mut frame_report = ProjectRuntimeSessionFrameReport::new(
            frame_index,
            self.project_runtime_session.session_id().to_string(),
        );
        frame_report.push_stage(stage_report);
        if terminal_fault {
            self.project_runtime_session_faulted = true;
        } else if let Some(contract) = self.project_observation_contract.as_ref() {
            let state = execute_project_runtime_observation(
                self.project_runtime_session.as_ref(),
                frame_index,
                time,
                world,
                contract,
                self.project_runtime_session_report_level,
            );
            frame_report.set_observation(&state);
            self.project_observation_state = Some(state);
        }
        let animator2d_command_count = self
            .frame_loop
            .apply_animator2d_commands(animator2d_commands);
        let project_runtime_session_report = match self.project_runtime_session_report_level {
            ProjectRuntimeSessionReportLevel::Off => None,
            ProjectRuntimeSessionReportLevel::Summary | ProjectRuntimeSessionReportLevel::Trace => {
                Some(frame_report)
            }
        };

        ImmediateAuiActionDispatchOutput {
            frame_index,
            runtime_advanced: false,
            project_runtime_session_report,
            project_observation_state: self.project_observation_state.clone(),
            animator2d_command_count,
            terminal_fault,
        }
    }

    pub fn set_animator2d_registry(
        &mut self,
        registry: crate::animator2d::CookedAnimator2DRegistry,
    ) -> Result<(), Vec<crate::animator2d::Animator2DDiagnostic>> {
        self.frame_loop.set_animator2d_registry(registry)
    }

    pub fn set_project_observation_contract(
        &mut self,
        contract: Option<CookedProjectObservationContract>,
    ) {
        self.project_observation_contract = contract;
        let contract = self.project_observation_contract.clone();
        self.reconcile_project_observation_contract(contract.as_ref());
    }

    pub fn clear_project_observation_state(&mut self) {
        self.project_observation_state = None;
    }

    pub fn render_scene_mut(&mut self) -> &mut RenderSceneState {
        &mut self.render_scene
    }

    pub fn render_thread_for_target(
        &mut self,
        render_target: RenderTarget,
    ) -> RenderThreadFrameOutput {
        self.render_thread_for_target_with_aui_overlay(render_target, None)
    }

    pub fn render_thread_for_target_with_aui_overlay(
        &mut self,
        render_target: RenderTarget,
        aui_overlay: Option<&AuiOverlayFrame>,
    ) -> RenderThreadFrameOutput {
        self.render_thread_for_target_with_aui_composition(render_target, aui_overlay, None)
    }

    pub fn render_thread_for_target_with_aui_composition(
        &mut self,
        render_target: RenderTarget,
        aui_overlay: Option<&AuiOverlayFrame>,
        aui_composition: Option<&AuiCompositionFrame>,
    ) -> RenderThreadFrameOutput {
        self.render_thread_for_target_with_runtime_resources(
            render_target,
            aui_overlay,
            aui_composition,
            None,
            None,
        )
    }

    pub fn render_thread_for_target_with_runtime_resources(
        &mut self,
        render_target: RenderTarget,
        aui_overlay: Option<&AuiOverlayFrame>,
        aui_composition: Option<&AuiCompositionFrame>,
        sprite_texture_bindings: Option<&Sprite2DTextureBindingContext>,
        runtime_texture_bindings: Option<&RuntimeTextureBindingContext>,
    ) -> RenderThreadFrameOutput {
        self.render_thread_for_target_with_runtime_resources_and_presentation(
            render_target,
            aui_overlay,
            aui_composition,
            sprite_texture_bindings,
            runtime_texture_bindings,
            None,
        )
    }

    pub fn render_thread_for_target_with_runtime_resources_and_presentation(
        &mut self,
        render_target: RenderTarget,
        aui_overlay: Option<&AuiOverlayFrame>,
        aui_composition: Option<&AuiCompositionFrame>,
        sprite_texture_bindings: Option<&Sprite2DTextureBindingContext>,
        runtime_texture_bindings: Option<&RuntimeTextureBindingContext>,
        game_view_presentation: Option<Arc<ResolvedGameViewPresentation>>,
    ) -> RenderThreadFrameOutput {
        let (ticket, immediate) = self
            .render_dispatcher
            .submit_frame_output(RenderFramePacket {
                frame_index: self.host_frame,
                render_scene_state: self.render_scene.clone(),
                render_frame_report: None,
                resource_requests: Vec::new(),
                resource_release_requests: Vec::new(),
                aui_overlay: aui_overlay.cloned(),
                aui_composition: aui_composition.cloned(),
                sprite_texture_bindings: sprite_texture_bindings.cloned(),
                runtime_texture_bindings: runtime_texture_bindings.cloned(),
                game_view_presentation,
                view_id: None,
                quality_profile: QualityProfile::default(),
                render_target,
            });
        if let Some((frame, _)) = immediate {
            return frame;
        }
        let _ = self
            .render_dispatcher
            .flush(RenderFenceSyncDepth::RenderThread);
        self.render_dispatcher
            .poll_submission_output(ticket)
            .map(|(frame, _)| frame)
            .expect("render thread frame should be available after flush")
    }

    pub fn tick(&mut self, input: EngineFrameInput, world: &mut World) -> EngineFrameOutput {
        self.tick_internal(input, world, None)
    }

    pub fn tick_with_runtime_context(
        &mut self,
        input: EngineFrameInput,
        world: &mut World,
        runtime_context: RuntimeFrameContext<'_>,
    ) -> EngineFrameOutput {
        self.tick_internal(input, world, Some(runtime_context))
    }

    fn tick_internal(
        &mut self,
        input: EngineFrameInput,
        world: &mut World,
        runtime_context: Option<RuntimeFrameContext<'_>>,
    ) -> EngineFrameOutput {
        self.host_frame += 1;
        if let Some(context) = runtime_context.as_ref() {
            self.project_observation_contract =
                context.package.manifest.observation_contract.clone();
        }
        let observation_contract = self.project_observation_contract.clone();
        self.reconcile_project_observation_contract(observation_contract.as_ref());
        let runtime_advanced = should_advance_runtime(input.mode);
        let render_built = should_build_render(input.mode);
        let aui_action_count = input
            .aui_interaction
            .as_ref()
            .map(|interaction| interaction.actions.len())
            .unwrap_or(0);

        if self.project_runtime_session_faulted {
            let mut trace = RuntimeTrace::new();
            trace.record(
                self.host_frame,
                "engine.engine_host_loop",
                "FrameFault",
                "project_runtime_session_reentry_rejected",
                Some(world.entity_count()),
            );
            return EngineFrameOutput {
                frame_index: self.host_frame,
                runtime_advanced: false,
                render_built: false,
                runtime_trace: trace,
                render_frame_report: None,
                renderer_feature_frame: None,
                minimal_renderer_frame: None,
                render_thread_frame: None,
                render_submission_report: None,
                render_worker_report: None,
                frame_hash: None,
                time_trace_summary: None,
                project_runtime_session_report: Some(
                    ProjectRuntimeSessionFrameReport::reentry_after_fault(
                        self.host_frame,
                        self.project_runtime_session.session_id().to_string(),
                    ),
                ),
                project_observation_state: self.project_observation_state.clone(),
                animator2d_frame_result: Animator2DFrameResult::default(),
            };
        }

        if !runtime_advanced {
            let mut trace = RuntimeTrace::new();
            trace.record(
                self.host_frame,
                "engine.engine_host_loop",
                "FrameBegin",
                "begin",
                Some(world.entity_count()),
            );
            trace.record(
                self.host_frame,
                "engine.engine_host_loop",
                "RuntimeFrame",
                "skipped",
                Some(world.entity_count()),
            );
            trace.record(
                self.host_frame,
                "engine.engine_host_loop",
                "FrameEnd",
                "end",
                Some(world.entity_count()),
            );
            return EngineFrameOutput {
                frame_index: self.host_frame,
                runtime_advanced: false,
                render_built: false,
                runtime_trace: trace,
                render_frame_report: None,
                renderer_feature_frame: None,
                minimal_renderer_frame: None,
                render_thread_frame: None,
                render_submission_report: None,
                render_worker_report: None,
                frame_hash: None,
                time_trace_summary: None,
                project_runtime_session_report: (aui_action_count > 0).then(|| {
                    ProjectRuntimeSessionFrameReport::discarded_non_advancing(
                        self.host_frame,
                        self.project_runtime_session.session_id().to_string(),
                        aui_action_count,
                    )
                }),
                project_observation_state: self.project_observation_state.clone(),
                animator2d_frame_result: Animator2DFrameResult::default(),
            };
        }

        let actions = input
            .aui_interaction
            .as_ref()
            .map(|interaction| interaction.actions.as_slice())
            .unwrap_or(&[]);
        let runtime_frame = self
            .frame_loop
            .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
                world,
                &mut self.render_scene,
                &mut self.extract,
                input.action_snapshot.as_ref(),
                input.input_trace_summary,
                input.unscaled_delta_time,
                runtime_context,
                ProjectRuntimeFrameSession {
                    session: self.project_runtime_session.as_mut(),
                    actions,
                    report_level: self.project_runtime_session_report_level,
                    observation_contract: self.project_observation_contract.as_ref(),
                },
                input.fixed_step_count,
            );
        let runtime_frame = match runtime_frame {
            Ok(frame) => frame,
            Err(fault) => {
                self.project_runtime_session_faulted = true;
                return EngineFrameOutput {
                    frame_index: fault.frame_index,
                    runtime_advanced: false,
                    render_built: false,
                    runtime_trace: fault.runtime_trace,
                    render_frame_report: None,
                    renderer_feature_frame: None,
                    minimal_renderer_frame: None,
                    render_thread_frame: None,
                    render_submission_report: None,
                    render_worker_report: None,
                    frame_hash: None,
                    time_trace_summary: Some(fault.time_trace_summary),
                    project_runtime_session_report: Some(fault.report),
                    project_observation_state: self.project_observation_state.clone(),
                    animator2d_frame_result: Animator2DFrameResult::default(),
                };
            }
        };

        self.sync_scene_camera_2d_view(world);

        self.output_from_runtime_frame(
            runtime_frame,
            render_built,
            input.aui_overlay,
            input.aui_composition,
            input.runtime_texture_bindings,
        )
    }

    fn sync_scene_camera_2d_view(&mut self, world: &World) {
        let Some((camera_id, half_height, camera_position)) = scene_camera_2d(world) else {
            return;
        };
        let mut view = self
            .render_scene
            .views()
            .find(|view| view.view_kind == RenderViewKind::Game)
            .cloned()
            .unwrap_or_else(|| {
                RenderViewState::new(
                    RenderViewId(1),
                    RenderViewKind::Game,
                    RenderTargetKind::ViewportTexture,
                )
            });
        let width = self.game_view_target.extent.width.max(1);
        let height = self.game_view_target.extent.height.max(1);
        let aspect = width as f32 / height as f32;
        let half_width = half_height * aspect;
        view.source_entity_id = Some(camera_id);
        view.viewport = Viewport {
            x: 0,
            y: 0,
            width,
            height,
        };
        view.view_matrix =
            translation_matrix(-camera_position.x, -camera_position.y, -camera_position.z);
        view.projection_matrix = orthographic_2d_projection(half_width, half_height);
        view.version = view.version.saturating_add(1);
        self.render_scene.register_view(view);
    }

    fn output_from_runtime_frame(
        &mut self,
        runtime_frame: RuntimeFrameOutput,
        render_built: bool,
        aui_overlay: Option<AuiOverlayFrame>,
        aui_composition: Option<AuiCompositionFrame>,
        runtime_texture_bindings: Option<RuntimeTextureBindingContext>,
    ) -> EngineFrameOutput {
        if let Some(state) = runtime_frame.project_observation_state.clone() {
            self.project_observation_state = Some(state);
        }
        let renderer_feature_frame = render_built.then(|| {
            self.feature_builder
                .build(runtime_frame.frame_index, &self.render_scene)
        });
        let minimal_renderer_frame = renderer_feature_frame
            .as_ref()
            .map(|feature_frame| self.minimal_renderer.render(feature_frame));
        let sprite_texture_bindings = runtime_texture_bindings.as_ref().map(|bindings| {
            let mut sprite_bindings = Sprite2DTextureBindingContext::new();
            for (asset_id, binding) in bindings.bindings() {
                sprite_bindings.insert_texture_handle(
                    asset_id,
                    binding.handle,
                    binding.sampler.clone(),
                );
            }
            sprite_bindings
        });
        let render_submission = render_built.then(|| {
            let (ticket, immediate) =
                self.render_dispatcher
                    .submit_frame_output(RenderFramePacket {
                        frame_index: runtime_frame.frame_index,
                        render_scene_state: self.render_scene.clone(),
                        render_frame_report: Some(runtime_frame.render_frame_report.clone()),
                        resource_requests: Vec::new(),
                        resource_release_requests: Vec::new(),
                        aui_overlay,
                        aui_composition,
                        sprite_texture_bindings,
                        runtime_texture_bindings,
                        game_view_presentation: None,
                        view_id: None,
                        quality_profile: QualityProfile::default(),
                        render_target: RenderTarget::viewport_texture(
                            "viewport-main",
                            self.game_view_target.extent.width,
                            self.game_view_target.extent.height,
                        )
                        .with_presentation_scale_policy(self.game_view_target.scale_policy),
                    });
            if let Some(submission) = immediate {
                return submission;
            }
            let _ = self
                .render_dispatcher
                .flush(RenderFenceSyncDepth::RenderThread);
            self.render_dispatcher
                .poll_submission_output(ticket)
                .expect("render submission should be available after flush")
        });
        let render_thread_frame = render_submission.as_ref().map(|(frame, _)| frame.clone());
        let render_submission_report = render_submission.map(|(_, report)| report);
        self.frame_lag_controller.update(
            runtime_frame.frame_index,
            self.render_dispatcher.completed_frame_index(),
        );
        let render_worker_report = render_built.then(|| self.render_dispatcher.worker_report());

        let animator2d_frame_result = runtime_frame.animator2d_frame_result;
        EngineFrameOutput {
            frame_index: runtime_frame.frame_index,
            runtime_advanced: true,
            render_built,
            runtime_trace: runtime_frame.runtime_trace,
            render_frame_report: Some(runtime_frame.render_frame_report),
            renderer_feature_frame,
            minimal_renderer_frame,
            render_thread_frame,
            render_submission_report,
            render_worker_report,
            frame_hash: Some(runtime_frame.frame_hash),
            time_trace_summary: Some(runtime_frame.time_trace_summary),
            project_runtime_session_report: match self.project_runtime_session_report_level {
                ProjectRuntimeSessionReportLevel::Off => None,
                ProjectRuntimeSessionReportLevel::Summary
                | ProjectRuntimeSessionReportLevel::Trace => {
                    runtime_frame.project_runtime_session_report
                }
            },
            project_observation_state: self.project_observation_state.clone(),
            animator2d_frame_result,
        }
    }

    fn reconcile_project_observation_contract(
        &mut self,
        contract: Option<&CookedProjectObservationContract>,
    ) {
        let Some(contract) = contract else {
            self.project_observation_state = None;
            return;
        };
        let session_id = self.project_runtime_session.session_id();
        let matches_active_identity =
            self.project_observation_state
                .as_ref()
                .is_some_and(|state| {
                    state.session_id() == session_id
                        && state.contract_digest() == contract.contract_digest
                });
        if !matches_active_identity {
            self.project_observation_state = Some(
                ProjectRuntimeObservationState::not_produced_yet(session_id, contract),
            );
        }
    }
}

fn scene_camera_2d(world: &World) -> Option<(crate::ids::SourceEntityId, f32, crate::math::Vec3)> {
    let component_type = ComponentTypeId::from("project.camera2d");
    world.entity_ids().into_iter().find_map(|entity_id| {
        let meta = world.entity(entity_id)?;
        if !meta.alive || !meta.enabled || meta.kind != "camera" {
            return None;
        }
        let ComponentValue::Dynamic {
            value: RuntimeValue::Object(fields),
            ..
        } = world.component_value(entity_id, &component_type)?
        else {
            return None;
        };
        let half_height = match fields.get("orthographicSize")? {
            RuntimeValue::F64(value) => *value as f32,
            RuntimeValue::I64(value) => *value as f32,
            _ => return None,
        };
        if !half_height.is_finite() || half_height <= 0.0 {
            return None;
        }
        Some((
            entity_id.clone(),
            half_height,
            world
                .transform(entity_id)
                .map(|transform| transform.local_position)
                .unwrap_or(crate::math::Vec3::ZERO),
        ))
    })
}

fn translation_matrix(x: f32, y: f32, z: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        x, y, z, 1.0,
    ]
}

fn orthographic_2d_projection(half_width: f32, half_height: f32) -> [f32; 16] {
    [
        1.0 / half_width,
        0.0,
        0.0,
        0.0, //
        0.0,
        1.0 / half_height,
        0.0,
        0.0, //
        0.0,
        0.0,
        1.0,
        0.0, //
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

fn should_advance_runtime(mode: EngineHostMode) -> bool {
    match mode {
        EngineHostMode::HeadlessServer
        | EngineHostMode::ExportedGame
        | EngineHostMode::EditorPlay
        | EngineHostMode::EditorStep => true,
        EngineHostMode::EditorPause => false,
    }
}

fn should_build_render(mode: EngineHostMode) -> bool {
    match mode {
        EngineHostMode::ExportedGame | EngineHostMode::EditorPlay | EngineHostMode::EditorStep => {
            true
        }
        EngineHostMode::HeadlessServer | EngineHostMode::EditorPause => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animator2d::{
        Animator2DParameterKind, Animator2DPlayback, Animator2DTransitionTiming,
        CookedAnimator2DCondition, CookedAnimator2DParameter, CookedAnimator2DRegistry,
        CookedAnimator2DState, CookedAnimator2DTransition, CookedAnimatorController2D,
        CookedSpriteAnimationClip2D, CookedSpriteAnimationFrame2D, RuntimeAnimator2D,
    };
    use crate::archetype::ComponentValue;
    use crate::aui::{AuiAction, AuiActionEvent};
    use crate::components::{ComponentTypeId, Hierarchy, Renderable, SpriteRenderer2D, Transform};
    use crate::ids::EntityId;
    use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
    use crate::math::Vec3;
    use crate::project_logic::{ProjectLogicRunner, RuleCall, RuleExecutionPlan};
    use crate::project_observation::{
        ProjectObservationContract, ProjectObservationEntry, ProjectObservationType,
        ProjectObservationValue, PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
    };
    use crate::project_runtime_session::{
        ProjectAuiActionBatch, ProjectRuntimeMutationBuffer, ProjectRuntimeObservationContext,
        ProjectRuntimeObservationOutput, ProjectRuntimeSessionContext, ProjectRuntimeSessionOutput,
        ProjectRuntimeSessionStatus,
    };
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::runtime_package::{
        load_runtime_package, RuntimeProjectInfo, RuntimeScene, RUNTIME_SCENE_SCHEMA_VERSION,
    };
    use crate::runtime_package_builder::{
        RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
        RuntimePackageBuilder, RuntimePackageSourceJson,
    };
    use crate::scene_loader::load_scene_into_world;
    use crate::scene_loader::tests_support::renderable_scene_fixture;
    use engine_input::InputMappingAsset;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct RecordingSessionState {
        action_calls: usize,
        fixed_calls: usize,
        action_ids: Vec<String>,
        fixed_observed_x: Vec<f32>,
        observation_calls: usize,
        observation_x: Vec<f32>,
    }

    struct RecordingSession {
        state: Arc<Mutex<RecordingSessionState>>,
        action_status: ProjectRuntimeSessionStatus,
        action_write_x: Option<f32>,
        fixed_write_x: Option<f32>,
        fault_on_fixed: bool,
    }

    struct InvalidMutationObservationSession {
        observation_calls: Arc<AtomicUsize>,
    }

    #[test]
    fn scene_camera_2d_updates_game_view_projection_from_hydrated_component() {
        let mut world = World::new();
        let camera_id = EntityId::from("camera-main");
        world
            .try_spawn_with_components(
                camera_id.clone(),
                "Main Camera",
                "camera",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                Some(Transform {
                    local_position: Vec3 {
                        x: 2.0,
                        y: -3.0,
                        z: 10.0,
                    },
                    local_rotation: Vec3::ZERO,
                    local_scale: Vec3::ONE,
                }),
                None,
            )
            .unwrap();
        world
            .try_insert_dynamic_component(
                camera_id.clone(),
                ComponentTypeId::from("project.camera2d"),
                RuntimeValue::object([("orthographicSize", RuntimeValue::F64(9.6))]),
            )
            .unwrap();
        let mut host = EngineHostLoop::new("scene-camera-test");
        host.set_game_view_target(GameViewTargetSpec::portrait_720x1280());

        host.sync_scene_camera_2d_view(&world);

        let view = host
            .render_scene()
            .views()
            .find(|view| view.view_kind == RenderViewKind::Game)
            .expect("scene camera Game view");
        assert_eq!(view.source_entity_id.as_ref(), Some(&camera_id));
        assert_eq!(view.viewport.width, 720);
        assert_eq!(view.viewport.height, 1280);
        assert!((view.projection_matrix[0] - 1.0 / 5.4).abs() < 0.0001);
        assert!((view.projection_matrix[5] - 1.0 / 9.6).abs() < 0.0001);
        assert_eq!(view.view_matrix[12], -2.0);
        assert_eq!(view.view_matrix[13], 3.0);
    }

    impl ProjectRuntimeSession for InvalidMutationObservationSession {
        fn session_id(&self) -> &str {
            "test.invalid-mutation.session"
        }

        fn handle_aui_actions(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
            batch: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut output = ProjectRuntimeSessionOutput::no_op();
            output.status = ProjectRuntimeSessionStatus::Unhandled;
            output.unhandled_action_count = batch.len();
            output
        }

        fn fixed_update(
            &mut self,
            _context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.write_transform(EntityId::from("missing-entity"), Transform::identity());
            ProjectRuntimeSessionOutput::applied(mutations)
        }

        fn observe(
            &self,
            _context: ProjectRuntimeObservationContext<'_>,
        ) -> ProjectRuntimeObservationOutput {
            self.observation_calls.fetch_add(1, Ordering::SeqCst);
            ProjectRuntimeObservationOutput::empty()
                .with_value("test.positionX", ProjectObservationValue::Number(99.0))
        }
    }

    impl ProjectRuntimeSession for RecordingSession {
        fn session_id(&self) -> &str {
            "test.recording.session"
        }

        fn handle_aui_actions(
            &mut self,
            context: ProjectRuntimeSessionContext<'_>,
            batch: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut state = self.state.lock().unwrap();
            state.action_calls += 1;
            state.action_ids.extend(
                batch
                    .actions()
                    .iter()
                    .map(|action| action.action_id.clone()),
            );
            drop(state);

            let mut mutations = ProjectRuntimeMutationBuffer::new();
            if let Some(x) = self.action_write_x {
                let mut transform = context
                    .world
                    .read_transform(&EntityId::from("entity-player"))
                    .unwrap();
                transform.local_position.x = x;
                mutations.write_transform(EntityId::from("entity-player"), transform);
            }
            let mut output = ProjectRuntimeSessionOutput::applied(mutations);
            output.status = self.action_status;
            match self.action_status {
                ProjectRuntimeSessionStatus::Applied => {
                    output.handled_action_count = batch.len();
                }
                ProjectRuntimeSessionStatus::Unhandled => {
                    output.unhandled_action_count = batch.len();
                }
                ProjectRuntimeSessionStatus::Rejected => {
                    output.rejected_action_count = batch.len();
                }
                ProjectRuntimeSessionStatus::Faulted => {
                    output.diagnostics.push("test.session.action_fault");
                }
                ProjectRuntimeSessionStatus::NoOp => {}
            }
            output
        }

        fn fixed_update(
            &mut self,
            context: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut transform = context
                .world
                .read_transform(&EntityId::from("entity-player"))
                .unwrap();
            let mut state = self.state.lock().unwrap();
            state.fixed_calls += 1;
            state.fixed_observed_x.push(transform.local_position.x);
            drop(state);
            if self.fault_on_fixed {
                let mut output = ProjectRuntimeSessionOutput::no_op();
                output.status = ProjectRuntimeSessionStatus::Faulted;
                output.diagnostics.push("test.session.fixed_fault");
                return output;
            }
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            if let Some(x) = self.fixed_write_x {
                transform.local_position.x = x;
                mutations.write_transform(EntityId::from("entity-player"), transform);
            }
            ProjectRuntimeSessionOutput::applied(mutations)
        }

        fn observe(
            &self,
            context: ProjectRuntimeObservationContext<'_>,
        ) -> ProjectRuntimeObservationOutput {
            let x = context
                .world
                .read_transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x;
            let mut state = self.state.lock().unwrap();
            state.observation_calls += 1;
            state.observation_x.push(x);
            drop(state);
            ProjectRuntimeObservationOutput::empty().with_value(
                "test.positionX",
                ProjectObservationValue::Number(f64::from(x)),
            )
        }
    }

    fn recording_host(
        project_logic: ProjectLogicRunner,
        action_status: ProjectRuntimeSessionStatus,
        action_write_x: Option<f32>,
        fixed_write_x: Option<f32>,
        fault_on_fixed: bool,
    ) -> (EngineHostLoop, Arc<Mutex<RecordingSessionState>>) {
        let state = Arc::new(Mutex::new(RecordingSessionState::default()));
        let session = RecordingSession {
            state: Arc::clone(&state),
            action_status,
            action_write_x,
            fixed_write_x,
            fault_on_fixed,
        };
        (
            EngineHostLoop::with_project_runtime_session(
                "scene-main",
                project_logic,
                Box::new(session),
            ),
            state,
        )
    }

    fn action(action_id: &str, payload: Option<&str>) -> AuiAction {
        AuiAction {
            action_id: action_id.to_string(),
            node_id: format!("node-{action_id}"),
            event: AuiActionEvent::Click,
            payload: payload.map(str::to_string),
        }
    }

    fn interaction(actions: Vec<AuiAction>) -> AuiInteractionResult {
        AuiInteractionResult {
            actions,
            ..AuiInteractionResult::default()
        }
    }

    const SESSION_ORDER_RULE: &str = "test.session_order_rule";

    fn session_order_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let entity_id = EntityId::from("entity-player");
        let mut position = context
            .read_transform_local_position(&entity_id)
            .expect("transform exists");
        position.x += 1.0;
        let write = context
            .write_transform_local_position(entity_id, position)
            .expect("write succeeds");
        let mut result = LogicResult::applied(SESSION_ORDER_RULE, ExecutorKind::RustAot);
        result.writes.push(write);
        result
    }

    fn fixed_order_runner() -> ProjectLogicRunner {
        let mut runner = ProjectLogicRunner::new(RuleExecutionPlan {
            fixed_update: vec![RuleCall::rust_aot(SESSION_ORDER_RULE)],
            frame_update: Vec::new(),
            post_physics: Vec::new(),
            event_handler: Vec::new(),
        });
        runner.register_rust_aot_rule(SESSION_ORDER_RULE, session_order_rule);
        runner
    }

    fn observation_runtime_package() -> crate::runtime_package::RuntimePackage {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let package_dir = std::env::temp_dir().join(format!("project-runtime-observation-{stamp}"));
        let request = RuntimePackageBuildRequest::dev_desktop(&package_dir, "scene-main");
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "project-observation-test",
            "Observation Test",
            "0.0.2",
        ));
        input.scenes.push(RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: "scene-main".to_string(),
            name: "Main".to_string(),
            gravity: 0.0,
            background: "#000000".to_string(),
            sky_color: "#000000".to_string(),
            entities: Vec::new(),
        });
        let mapping = InputMappingAsset::explicit_empty("input.none");
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        input.observation_contract = Some(ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.to_string(),
            contract_id: "test.runtime-observations".to_string(),
            observations: vec![ProjectObservationEntry {
                path: "test.positionX".to_string(),
                value_type: ProjectObservationType::Number,
                description: "Post-commit player x position".to_string(),
                allowed_values: None,
            }],
        });
        let report = RuntimePackageBuilder::build(&request, &input);
        assert_eq!(report.status, RuntimePackageBuildStatus::Success);
        load_runtime_package(&package_dir).value.unwrap()
    }

    fn tick_with_observation_package(
        host: &mut EngineHostLoop,
        input: EngineFrameInput,
        world: &mut World,
        package: &crate::runtime_package::RuntimePackage,
        instance_loader: &mut RuntimeInstanceLoader,
    ) -> EngineFrameOutput {
        host.tick_with_runtime_context(
            input,
            world,
            RuntimeFrameContext {
                package,
                instance_loader,
            },
        )
    }

    #[test]
    fn project_runtime_session_host_one_click_dispatches_once_and_empty_frame_does_not_replay() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer)
                .with_aui_interaction(interaction(vec![action("ui.once", None)])),
            &mut world,
        );
        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );

        let state = state.lock().unwrap();
        assert_eq!(state.action_calls, 1);
        assert_eq!(state.action_ids, vec!["ui.once"]);
        assert_eq!(state.fixed_calls, 2);
    }

    #[test]
    fn immediate_aui_action_dispatch_does_not_advance_runtime() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            Some(7.0),
            None,
            false,
        );
        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );
        let before = host.frame_loop.runtime_time().trace_summary();

        let output =
            host.dispatch_aui_actions_immediately(&[action("ui.immediate", None)], &mut world);

        let after = host.frame_loop.runtime_time().trace_summary();
        assert!(!output.runtime_advanced);
        assert_eq!(before.frame_count, after.frame_count);
        assert_eq!(before.fixed_frame_count, after.fixed_frame_count);
        assert_eq!(
            world
                .transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            7.0
        );
        let state = state.lock().unwrap();
        assert_eq!(state.action_calls, 1);
        assert_eq!(state.fixed_calls, 1);
        assert_eq!(state.action_ids, vec!["ui.immediate"]);
    }

    #[test]
    fn project_runtime_session_host_multiple_actions_preserve_vector_order() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_aui_interaction(
                interaction(vec![
                    action("ui.a", None),
                    action("ui.b", None),
                    action("ui.c", None),
                ]),
            ),
            &mut world,
        );

        assert_eq!(
            state.lock().unwrap().action_ids,
            vec!["ui.a", "ui.b", "ui.c"]
        );
    }

    #[test]
    fn project_runtime_session_host_rejected_and_unhandled_actions_do_not_replay() {
        for status in [
            ProjectRuntimeSessionStatus::Rejected,
            ProjectRuntimeSessionStatus::Unhandled,
        ] {
            let scene = renderable_scene_fixture();
            let mut world = load_scene_into_world(&scene).value.unwrap();
            let (mut host, state) =
                recording_host(ProjectLogicRunner::empty(), status, None, None, false);
            host.tick(
                EngineFrameInput::new(EngineHostMode::HeadlessServer)
                    .with_aui_interaction(interaction(vec![action("ui.drop", None)])),
                &mut world,
            );
            host.tick(
                EngineFrameInput::new(EngineHostMode::HeadlessServer),
                &mut world,
            );
            assert_eq!(state.lock().unwrap().action_calls, 1);
        }
    }

    #[test]
    fn project_runtime_session_host_non_advancing_frame_discards_without_queue() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );

        let paused = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause)
                .with_aui_interaction(interaction(vec![action("ui.pause", None)])),
            &mut world,
        );
        host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );

        let report = paused.project_runtime_session_report.unwrap();
        assert_eq!(report.status, "discarded_non_advancing_mode");
        assert_eq!(report.discarded_action_count, 1);
        let state = state.lock().unwrap();
        assert_eq!(state.action_calls, 0);
        assert_eq!(state.fixed_calls, 1);
    }

    #[test]
    fn project_runtime_session_host_editor_step_calls_action_and_fixed_once() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep)
                .with_aui_interaction(interaction(vec![action("ui.step", None)])),
            &mut world,
        );

        let state = state.lock().unwrap();
        assert_eq!(state.action_calls, 1);
        assert_eq!(state.fixed_calls, 1);
    }

    #[test]
    fn animator2d_schedule_host_pause_step_and_two_fixed_ticks() {
        let registry = animator_host_registry();
        let mut world = animator_host_world(&registry.registry_digest);
        let mut host = EngineHostLoop::with_project_runtime_session(
            "animator-host",
            ProjectLogicRunner::empty(),
            Box::new(AnimatorHostSession),
        );
        host.frame_loop.set_animator2d_registry(registry).unwrap();

        let paused = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
        );
        assert!(!paused.runtime_advanced);
        assert_eq!(
            host.frame_loop
                .animator2d_module()
                .unwrap()
                .instance_count(),
            0
        );
        assert_eq!(
            world
                .sprite_renderer2d(&EntityId::from("animated"))
                .unwrap()
                .sprite_ref,
            None
        );

        let stepped = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        assert!(stepped.runtime_advanced);
        assert_eq!(stepped.time_trace_summary.unwrap().fixed_frame_count, 1);
        assert_eq!(
            world
                .sprite_renderer2d(&EntityId::from("animated"))
                .unwrap()
                .sprite_ref
                .as_deref(),
            Some("attack-0")
        );

        let second = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPlay),
            &mut world,
        );
        assert_eq!(second.time_trace_summary.unwrap().fixed_frame_count, 2);
    }

    #[test]
    fn animator2d_schedule_large_delta_is_bounded_to_one_formal_fixed_tick() {
        let registry = animator_host_registry();
        let mut world = animator_host_world(&registry.registry_digest);
        let mut host = EngineHostLoop::with_project_runtime_session(
            "animator-host",
            ProjectLogicRunner::empty(),
            Box::new(AnimatorHostSession),
        );
        host.frame_loop.set_animator2d_registry(registry).unwrap();

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPlay).with_unscaled_delta_time(10.0),
            &mut world,
        );

        let summary = output.time_trace_summary.unwrap();
        assert!(summary.clamped_by_maximum_delta_time);
        assert_eq!(summary.fixed_frame_count, 1);
        assert_eq!(
            host.frame_loop
                .animator2d_module()
                .unwrap()
                .entity_state(&EntityId::from("animated"))
                .unwrap()
                .state_id,
            "attack"
        );
    }

    #[test]
    fn animator2d_schedule_host_replacement_does_not_retain_instance_memory() {
        let registry = animator_host_registry();
        let mut world = animator_host_world(&registry.registry_digest);
        let mut first = EngineHostLoop::with_project_runtime_session(
            "animator-host",
            ProjectLogicRunner::empty(),
            Box::new(AnimatorHostSession),
        );
        first
            .frame_loop
            .set_animator2d_registry(registry.clone())
            .unwrap();
        first.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        assert_eq!(
            first
                .frame_loop
                .animator2d_module()
                .unwrap()
                .instance_count(),
            1
        );

        let mut replacement = EngineHostLoop::with_project_runtime_session(
            "animator-host",
            ProjectLogicRunner::empty(),
            Box::new(AnimatorHostSession),
        );
        replacement
            .frame_loop
            .set_animator2d_registry(registry)
            .unwrap();
        assert_eq!(
            replacement
                .frame_loop
                .animator2d_module()
                .unwrap()
                .instance_count(),
            0
        );
        replacement.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
        );
        assert_eq!(
            replacement
                .frame_loop
                .animator2d_module()
                .unwrap()
                .instance_count(),
            0
        );
    }

    struct AnimatorHostSession;

    impl ProjectRuntimeSession for AnimatorHostSession {
        fn session_id(&self) -> &str {
            "animator.host.session"
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
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.animator2d_set_trigger(EntityId::from("animated"), "attack");
            ProjectRuntimeSessionOutput::applied(mutations)
        }
    }

    fn animator_host_world(registry_digest: &str) -> World {
        let mut world = World::new();
        let entity_id = EntityId::from("animated");
        world
            .try_spawn_entity(
                entity_id.clone(),
                "Animated",
                "actor",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
            )
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

    fn animator_host_registry() -> CookedAnimator2DRegistry {
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
                    priority: 10,
                    conditions: vec![CookedAnimator2DCondition::Triggered { parameter_index: 0 }],
                }],
            }],
        )
        .unwrap()
    }

    #[test]
    fn project_runtime_session_host_fixed_update_runs_before_project_logic_fixed_update() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            fixed_order_runner(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            Some(5.0),
            false,
        );

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );

        assert_eq!(state.lock().unwrap().fixed_calls, 1);
        assert_eq!(
            world
                .transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            6.0
        );
        let session_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| {
                event.system_id == "project.runtime_session" && event.phase == "fixed_update"
            })
            .unwrap();
        let rule_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == format!("project.rule.{SESSION_ORDER_RULE}"))
            .unwrap();
        assert!(session_index < rule_index);
    }

    #[test]
    fn project_runtime_session_observation_publishes_only_post_commit_final_state() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            fixed_order_runner(),
            ProjectRuntimeSessionStatus::Applied,
            Some(3.0),
            Some(5.0),
            false,
        );
        let package = observation_runtime_package();
        let mut loader = RuntimeInstanceLoader::from_package(&package);

        let output = tick_with_observation_package(
            &mut host,
            EngineFrameInput::new(EngineHostMode::HeadlessServer)
                .with_aui_interaction(interaction(vec![action("ui.commit", None)])),
            &mut world,
            &package,
            &mut loader,
        );

        let recorded = state.lock().unwrap();
        assert_eq!(recorded.action_calls, 1);
        assert_eq!(recorded.fixed_calls, 1);
        assert_eq!(recorded.observation_calls, 1);
        assert_eq!(recorded.observation_x, vec![6.0]);
        drop(recorded);
        let ProjectRuntimeObservationState::Published { snapshot } = output
            .project_observation_state
            .expect("published observation")
        else {
            panic!("expected published observation");
        };
        assert_eq!(snapshot.runtime_frame, output.frame_index);
        assert_eq!(
            snapshot.values.get("test.positionX"),
            Some(&ProjectObservationValue::Number(6.0))
        );
        let post_commit_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.phase == "RuntimeFramePostCommit")
            .unwrap();
        let rule_index = output
            .runtime_trace
            .events
            .iter()
            .position(|event| event.system_id == format!("project.rule.{SESSION_ORDER_RULE}"))
            .unwrap();
        assert!(rule_index < post_commit_index);
    }

    #[test]
    fn project_runtime_session_observation_pause_step_and_clear_follow_lifecycle() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );
        let package = observation_runtime_package();
        let mut loader = RuntimeInstanceLoader::from_package(&package);

        let paused_before_first = tick_with_observation_package(
            &mut host,
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
            &package,
            &mut loader,
        );
        assert!(matches!(
            paused_before_first.project_observation_state,
            Some(ProjectRuntimeObservationState::NotProducedYet { .. })
        ));
        assert_eq!(state.lock().unwrap().observation_calls, 0);

        let stepped = tick_with_observation_package(
            &mut host,
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
            &package,
            &mut loader,
        );
        let stepped_frame = stepped
            .project_observation_state
            .as_ref()
            .and_then(ProjectRuntimeObservationState::runtime_frame)
            .unwrap();
        assert_eq!(state.lock().unwrap().observation_calls, 1);

        let paused_after_step = tick_with_observation_package(
            &mut host,
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
            &package,
            &mut loader,
        );
        assert_eq!(
            paused_after_step
                .project_observation_state
                .as_ref()
                .and_then(ProjectRuntimeObservationState::runtime_frame),
            Some(stepped_frame)
        );
        assert_eq!(state.lock().unwrap().observation_calls, 1);

        host.clear_project_observation_state();
        assert!(host.project_observation_state().is_none());
    }

    #[test]
    fn project_runtime_session_observation_is_not_published_after_terminal_mutation_fault() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let observation_calls = Arc::new(AtomicUsize::new(0));
        let mut host = EngineHostLoop::with_project_runtime_session(
            "scene-main",
            ProjectLogicRunner::empty(),
            Box::new(InvalidMutationObservationSession {
                observation_calls: Arc::clone(&observation_calls),
            }),
        );
        let package = observation_runtime_package();
        let mut loader = RuntimeInstanceLoader::from_package(&package);

        let output = tick_with_observation_package(
            &mut host,
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
            &package,
            &mut loader,
        );

        assert!(!output.runtime_advanced);
        assert_eq!(observation_calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            output.project_observation_state,
            Some(ProjectRuntimeObservationState::NotProducedYet { .. })
        ));
    }

    #[test]
    fn project_runtime_session_host_action_mutation_is_visible_to_same_frame_fixed_update() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            Some(3.0),
            None,
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer)
                .with_aui_interaction(interaction(vec![action("ui.write", None)])),
            &mut world,
        );

        assert_eq!(state.lock().unwrap().fixed_observed_x, vec![3.0]);
    }

    #[test]
    fn project_runtime_session_host_fixed_mutation_is_visible_to_project_rule() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, _) = recording_host(
            fixed_order_runner(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            Some(9.0),
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );

        assert_eq!(
            world
                .transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            10.0
        );
    }

    #[test]
    fn project_runtime_session_host_terminal_fault_skips_later_phases() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let x_before = world
            .transform(&EntityId::from("entity-player"))
            .unwrap()
            .local_position
            .x;
        let (mut host, _) = recording_host(
            fixed_order_runner(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            true,
        );

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
        );

        assert!(!output.runtime_advanced);
        assert!(!output.render_built);
        assert!(
            output
                .project_runtime_session_report
                .as_ref()
                .unwrap()
                .terminal_fault
        );
        assert!(!output.runtime_trace.events.iter().any(|event| event
            .system_id
            .starts_with("project.rule.")
            || event.system_id == "engine.physics2d"
            || event.system_id == "engine.render_extract"));
        assert_eq!(
            world
                .transform(&EntityId::from("entity-player"))
                .unwrap()
                .local_position
                .x,
            x_before
        );
    }

    #[test]
    fn project_runtime_session_host_faulted_instance_rejects_future_reentry() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, state) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Faulted,
            None,
            None,
            false,
        );

        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer)
                .with_aui_interaction(interaction(vec![action("ui.fault", None)])),
            &mut world,
        );
        let second = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer)
                .with_aui_interaction(interaction(vec![action("ui.again", None)])),
            &mut world,
        );

        assert_eq!(state.lock().unwrap().action_calls, 1);
        assert!(second.project_runtime_session_report.unwrap().stages[0]
            .diagnostics
            .contains(&"project_runtime.session_reentry_after_fault".to_string()));
    }

    #[test]
    fn project_runtime_session_host_off_summary_trace_report_boundaries() {
        for level in [
            ProjectRuntimeSessionReportLevel::Off,
            ProjectRuntimeSessionReportLevel::Summary,
            ProjectRuntimeSessionReportLevel::Trace,
        ] {
            let scene = renderable_scene_fixture();
            let mut world = load_scene_into_world(&scene).value.unwrap();
            let (mut host, _) = recording_host(
                ProjectLogicRunner::empty(),
                ProjectRuntimeSessionStatus::Applied,
                None,
                None,
                false,
            );
            host.set_project_runtime_session_report_level(level);
            let output = host.tick(
                EngineFrameInput::new(EngineHostMode::HeadlessServer).with_aui_interaction(
                    interaction(vec![action("ui.trace", Some("{\"value\":7}"))]),
                ),
                &mut world,
            );
            match level {
                ProjectRuntimeSessionReportLevel::Off => {
                    assert!(output.project_runtime_session_report.is_none());
                }
                ProjectRuntimeSessionReportLevel::Summary => {
                    assert!(output.project_runtime_session_report.unwrap().stages[0]
                        .action_trace
                        .is_empty());
                }
                ProjectRuntimeSessionReportLevel::Trace => {
                    let trace =
                        &output.project_runtime_session_report.unwrap().stages[0].action_trace[0];
                    assert_eq!(trace.payload_byte_length, 11);
                    assert!(trace.payload_digest.is_some());
                }
            }
        }
    }

    #[test]
    fn project_runtime_session_host_summary_excludes_raw_payload() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let (mut host, _) = recording_host(
            ProjectLogicRunner::empty(),
            ProjectRuntimeSessionStatus::Applied,
            None,
            None,
            false,
        );
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_aui_interaction(
                interaction(vec![action("ui.secret", Some("raw-secret-payload"))]),
            ),
            &mut world,
        );

        let report = output.project_runtime_session_report.unwrap();
        assert!(report.stages[0].action_trace.is_empty());
        assert!(!format!("{report:?}").contains("raw-secret-payload"));
    }

    fn mark_renderable_dirty(world: &mut World) {
        world.insert_renderable(
            EntityId::from("entity-player"),
            Renderable {
                mesh_ref: Some("model-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            },
        );
    }

    fn move_player(world: &mut World, x: f32) {
        world.insert_transform(
            EntityId::from("entity-player"),
            Transform {
                local_position: Vec3 { x, y: 1.0, z: 2.0 },
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
        );
    }

    #[test]
    fn engine_host_loop_headless_advances_runtime() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        move_player(&mut world, 2.0);
        let mut host = EngineHostLoop::new(scene.id);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );

        assert!(output.runtime_advanced);
        assert!(!output.render_built);
        assert!(output.frame_hash.is_some());
        assert!(output.renderer_feature_frame.is_none());
    }

    #[test]
    fn engine_host_loop_pause_does_not_advance_runtime() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        move_player(&mut world, 2.0);
        let mut host = EngineHostLoop::new(scene.id);
        let dirty_before = world.dirty_records().len();

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
        );

        assert!(!output.runtime_advanced);
        assert!(!output.render_built);
        assert!(output.frame_hash.is_none());
        assert_eq!(world.dirty_records().len(), dirty_before);
    }

    #[test]
    fn engine_host_loop_step_advances_exactly_one_runtime_frame() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        move_player(&mut world, 2.0);
        let mut host = EngineHostLoop::new(scene.id);

        let first = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        let paused = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause),
            &mut world,
        );

        assert_eq!(first.frame_index, 1);
        assert!(first.runtime_advanced);
        assert!(!paused.runtime_advanced);
    }

    #[test]
    fn engine_host_loop_exported_game_produces_renderer_feature_frame() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        mark_renderable_dirty(&mut world);
        let mut host = EngineHostLoop::new(scene.id);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
        );

        assert!(output.runtime_advanced);
        assert!(output.render_built);
        assert_eq!(
            output
                .renderer_feature_frame
                .as_ref()
                .expect("feature frame")
                .draw_items
                .len(),
            1
        );
        assert_eq!(
            output
                .minimal_renderer_frame
                .as_ref()
                .expect("minimal renderer frame")
                .draw_record_count,
            1
        );
        let render_thread_frame = output
            .render_thread_frame
            .as_ref()
            .expect("render thread frame");
        assert_eq!(
            render_thread_frame.report.schema_version,
            "render-thread-report.v1"
        );
        assert_eq!(render_thread_frame.report.scene_proxy_count, 1);
        assert_eq!(render_thread_frame.report.present_status, "presented");
        assert_eq!(
            render_thread_frame
                .report
                .texture_lifetime_report
                .schema_version,
            "gpu-texture-lifetime-report.v1"
        );
        assert!(output.render_frame_report.is_some());
    }

    #[test]
    fn engine_host_loop_submits_render_frame_to_render_thread() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        mark_renderable_dirty(&mut world);
        let mut host = EngineHostLoop::new(scene.id);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        let render_thread_frame = output
            .render_thread_frame
            .as_ref()
            .expect("render thread frame");

        assert!(output.render_built);
        assert_eq!(
            render_thread_frame.report.thread_mode,
            crate::render_thread::RenderThreadMode::InlineDeterministic
        );
        assert_eq!(render_thread_frame.report.rdg_status, "ok");
        assert_eq!(render_thread_frame.report.rhi_status, "ok");
        assert!(
            render_thread_frame
                .report
                .texture_lifetime_report
                .event_count
                >= 5
        );
    }

    #[test]
    fn engine_host_loop_can_submit_to_dedicated_render_worker() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        mark_renderable_dirty(&mut world);
        let mut host =
            EngineHostLoop::new_with_render_mode(scene.id, RenderThreadMode::DedicatedThread);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        let submission = output
            .render_submission_report
            .as_ref()
            .expect("dedicated worker submission report");
        let worker = output
            .render_worker_report
            .as_ref()
            .expect("render worker report");

        assert!(output.render_thread_frame.is_some());
        assert_eq!(submission.thread_mode, RenderThreadMode::DedicatedThread);
        assert!(submission.presented);
        assert!(!submission
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "dedicated_thread_not_spawned_c_min"));
        assert_eq!(worker.last_completed_frame, 1);
    }

    #[test]
    fn engine_host_loop_passes_unscaled_delta_to_runtime_time() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        let mut host = EngineHostLoop::new(scene.id);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_unscaled_delta_time(0.2),
            &mut world,
        );

        let summary = output.time_trace_summary.expect("time summary");
        assert_eq!(summary.frame_count, 1);
        assert_eq!(summary.unscaled_delta_time, 0.2);
        assert_eq!(summary.delta_time, 0.2);
    }
}
