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
    execute_project_runtime_observation, execute_project_runtime_session_stage_with_media,
    EmptyProjectRuntimeSession, ProjectRuntimeSession, ProjectRuntimeSessionFrameReport,
    ProjectRuntimeSessionReportLevel, ProjectRuntimeSessionStage,
};
use crate::render_command::RenderFrameReport;
use crate::render_extract::RenderExtractContext;
use crate::render_state::{
    RenderSceneState, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState, Viewport,
};
use crate::render_thread::{
    RenderFramePacket, RenderSubmissionReport, RenderThreadConfig, RenderThreadDiagnostic,
    RenderThreadFrameOutput, RenderThreadMode,
};
use crate::render_thread_worker::{
    FrameLagController, RenderCommandDispatcher, RenderWorkerReport,
};
use crate::renderer_feature_builder::{RendererFeatureBuilder, RendererFeatureFrame};
use crate::runtime_renderer::{QualityProfile, RenderTarget};
use crate::runtime_texture::RuntimeTextureBindingContext;
use crate::runtime_time::{TimeTraceSummary, DEFAULT_FIXED_DELTA_TIME};
use crate::runtime_trace::RuntimeTrace;
use crate::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use crate::world::World;
use project_runtime_sdk::ProjectRuntimeHostControl;
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
    pub focus_changes: Vec<bool>,
    pub mode: EngineHostMode,
    pub action_snapshot: Option<ActionSnapshot>,
    pub input_trace_summary: Option<InputTraceSummary>,
    pub aui_overlay: Option<AuiOverlayFrame>,
    pub aui_composition: Option<AuiCompositionFrame>,
    pub aui_interaction: Option<AuiInteractionResult>,
    pub runtime_texture_bindings: Option<RuntimeTextureBindingContext>,
    pub unscaled_delta_time: f32,
    pub fixed_step_count: usize,
    pub fixed_steps_explicit: bool,
    pub build_render_submission: bool,
}

impl EngineFrameInput {
    pub fn new(mode: EngineHostMode) -> Self {
        Self {
            mode,
            focus_changes: Vec::new(),
            action_snapshot: None,
            input_trace_summary: None,
            aui_overlay: None,
            aui_composition: None,
            aui_interaction: None,
            runtime_texture_bindings: None,
            unscaled_delta_time: DEFAULT_FIXED_DELTA_TIME,
            fixed_step_count: 1,
            fixed_steps_explicit: false,
            build_render_submission: true,
        }
    }

    pub fn with_focus_changes(mut self, changes: Vec<bool>) -> Self {
        self.focus_changes = changes;
        self
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
        self.fixed_steps_explicit = true;
        self
    }

    pub fn with_deferred_render_submission(mut self) -> Self {
        self.build_render_submission = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineFrameOutput {
    pub host_control: Option<ProjectRuntimeHostControl>,
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
    pub host_control: Option<ProjectRuntimeHostControl>,
    pub frame_index: u64,
    pub runtime_advanced: bool,
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    pub project_observation_state: Option<ProjectRuntimeObservationState>,
    pub animator2d_command_count: usize,
    pub terminal_fault: bool,
}

#[derive(Debug, Clone)]
struct AuxiliaryCameraView {
    view: crate::render_state::RenderViewId,
    viewport: crate::render_state::Viewport,
    kind: crate::render_state::RenderViewKind,
    target: crate::render_state::RenderTargetKind,
}

pub struct EngineHostLoop {
    run_epoch: u64,
    frame_source: Option<Arc<crate::render_thread::RenderFrameSourceIdentity>>,
    frame_identity: Option<Arc<crate::render_thread::RenderFrameIdentity>>,
    audio: crate::runtime_audio::RuntimeAudio,
    pending_audio_commands: Vec<crate::runtime_audio::AudioSourceCommand>,
    pending_particle_commands: Vec<crate::runtime_particles::ParticleCommand>,
    particles: crate::runtime_particles::RuntimeParticles,
    frame_loop: FrameLoop,
    project_runtime_session: Box<dyn ProjectRuntimeSession>,
    project_runtime_session_report_level: ProjectRuntimeSessionReportLevel,
    project_runtime_session_faulted: bool,
    host_control: Option<ProjectRuntimeHostControl>,
    focused: Option<bool>,
    project_observation_contract: Option<CookedProjectObservationContract>,
    project_observation_state: Option<ProjectRuntimeObservationState>,
    render_scene: RenderSceneState,
    extract: RenderExtractContext,
    geometry_cache: crate::render_geometry::ProductionGeometryCache,
    geometry_snapshot: Option<Arc<crate::render_geometry::ProductionGeometrySnapshot>>,
    geometry_view_targets: std::collections::BTreeMap<u64, crate::runtime_renderer::RenderTarget>,
    geometry_output_revision: u64,
    auxiliary_camera_views:
        std::collections::BTreeMap<crate::ids::SourceEntityId, AuxiliaryCameraView>,
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
    #[cfg(all(test, feature = "real-wgpu"))]
    pub(crate) fn generated_collision_query_for_test(
        &mut self,
        world: &World,
        batch: &project_runtime_sdk::physics_query3d::PhysicsQueryBatch3D,
    ) -> Result<
        project_runtime_sdk::physics_query3d::PhysicsQueryResult3D,
        project_runtime_abi::ProjectRuntimeAbiStatus,
    > {
        self.frame_loop.physics3d_mut().query_batch3d(world, batch)
    }
    /// Select checked-plan production for an external GPU consumer on the inline dispatcher.
    pub fn set_external_rhi_enabled(&mut self, enabled: bool) -> bool {
        self.render_dispatcher.set_external_rhi_enabled(enabled)
    }
    pub fn set_ui_measurement_enabled(&self, enabled: bool) -> bool {
        self.render_dispatcher.set_ui_measurement_enabled(enabled)
    }

    pub fn ui_projection_stats(&self) -> Option<crate::runtime_renderer::UiProjectionStats> {
        self.render_dispatcher.ui_projection_stats()
    }
    pub fn new(scene_id: impl Into<String>) -> Self {
        Self {
            audio: crate::runtime_audio::RuntimeAudio::default(),
            pending_audio_commands: Vec::new(),
            pending_particle_commands: Vec::new(),
            particles: crate::runtime_particles::RuntimeParticles::default(),
            frame_loop: FrameLoop::new(scene_id),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            host_control: None,
            focused: None,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            geometry_cache: Default::default(),
            geometry_snapshot: None,
            geometry_view_targets: Default::default(),
            geometry_output_revision: 0,
            run_epoch: crate::render_thread::new_source_epoch(),
            frame_source: None,
            frame_identity: None,
            auxiliary_camera_views: std::collections::BTreeMap::new(),
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
            audio: crate::runtime_audio::RuntimeAudio::default(),
            pending_audio_commands: Vec::new(),
            pending_particle_commands: Vec::new(),
            particles: crate::runtime_particles::RuntimeParticles::default(),
            frame_loop: FrameLoop::with_project_logic(scene_id, project_logic),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            host_control: None,
            focused: None,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            geometry_cache: Default::default(),
            geometry_snapshot: None,
            geometry_view_targets: Default::default(),
            geometry_output_revision: 0,
            run_epoch: crate::render_thread::new_source_epoch(),
            frame_source: None,
            frame_identity: None,
            auxiliary_camera_views: std::collections::BTreeMap::new(),
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
            audio: crate::runtime_audio::RuntimeAudio::default(),
            pending_audio_commands: Vec::new(),
            pending_particle_commands: Vec::new(),
            particles: crate::runtime_particles::RuntimeParticles::default(),
            frame_loop: FrameLoop::with_project_logic(scene_id, project_logic),
            project_runtime_session,
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            host_control: None,
            focused: None,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            geometry_cache: Default::default(),
            geometry_snapshot: None,
            geometry_view_targets: Default::default(),
            geometry_output_revision: 0,
            run_epoch: crate::render_thread::new_source_epoch(),
            frame_source: None,
            frame_identity: None,
            auxiliary_camera_views: std::collections::BTreeMap::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::inline(RenderThreadConfig::default()),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    /// Read-only committed UI state, shared by native presentation consumers.
    pub fn retained_ui(&self) -> Option<&crate::aui_session::AuiSession> {
        self.project_runtime_session.retained_ui()
    }
    pub fn observe_project_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        self.project_runtime_session.observe_memory(sample);
    }
    pub fn publish_ui_geometry(&mut self, output: &crate::aui::AuiRuntimePresentOutput) {
        self.project_runtime_session.publish_ui_geometry(output);
    }
    pub fn advance_ui_animations(
        &mut self,
        frame: u64,
        delta_us: u64,
    ) -> Result<(), project_runtime_sdk::aui::UiError> {
        self.project_runtime_session
            .advance_ui_animations(frame, delta_us)
    }
    pub fn cancel_drag_animations(
        &mut self,
        document: &str,
        interaction: &crate::aui::AuiInteractionResult,
    ) {
        self.project_runtime_session
            .cancel_drag_animations(document, interaction);
    }

    pub fn aui_extension_stats(
        &self,
    ) -> crate::project_runtime_session::ProjectRuntimeAuiExtensionStats {
        self.project_runtime_session.aui_extension_stats()
    }

    pub fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &crate::aui_extension::AuiCustomFrame,
        allow_publish: bool,
    ) -> Result<crate::aui_extension::AuiCustomFrameResult, String> {
        self.project_runtime_session
            .refresh_ui_extensions_for_frame(frame, allow_publish)
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
            audio: crate::runtime_audio::RuntimeAudio::default(),
            pending_audio_commands: Vec::new(),
            pending_particle_commands: Vec::new(),
            particles: crate::runtime_particles::RuntimeParticles::default(),
            frame_loop: FrameLoop::new(scene_id),
            project_runtime_session: Box::new(EmptyProjectRuntimeSession),
            project_runtime_session_report_level: ProjectRuntimeSessionReportLevel::Off,
            project_runtime_session_faulted: false,
            host_control: None,
            focused: None,
            project_observation_contract: None,
            project_observation_state: None,
            render_scene: RenderSceneState::new(),
            extract: RenderExtractContext::new(),
            geometry_cache: Default::default(),
            geometry_snapshot: None,
            geometry_view_targets: Default::default(),
            geometry_output_revision: 0,
            run_epoch: crate::render_thread::new_source_epoch(),
            frame_source: None,
            frame_identity: None,
            auxiliary_camera_views: std::collections::BTreeMap::new(),
            feature_builder: RendererFeatureBuilder::new(),
            minimal_renderer: MinimalRenderer::new(),
            render_dispatcher: RenderCommandDispatcher::from_thread_mode(mode),
            frame_lag_controller: FrameLagController::default(),
            game_view_target: GameViewTargetSpec::default(),
            host_frame: 0,
        }
    }

    /// Explicit frame capture; disabled frames clear the optional Spine sample.
    pub fn begin_spine_measurement_frame(&self, enabled: bool) {
        if let Some(module) = self.frame_loop.spine_module() {
            module.begin_measurement_frame(enabled);
        }
    }
    pub fn spine_cpu_measurement(
        &self,
    ) -> Option<crate::spine_animation::measurement::SpineCpuFrameSample> {
        self.frame_loop
            .spine_module()
            .and_then(|m| m.cpu_measurement())
    }
    pub fn observe_spine(
        &self,
        level: crate::render_contract::DiagnosticLevel,
    ) -> Option<crate::spine_animation::session::SpineObservation> {
        self.frame_loop
            .spine_module()
            .and_then(|module| module.observe(level))
    }

    /// Explicit diagnostic request; normal frames never build spatial reports.
    pub fn observe_spatial(
        &self,
        world: &World,
        ids: &[String],
        level: crate::render_contract::DiagnosticLevel,
    ) -> Option<crate::render_geometry::SpatialObservation> {
        self.observe_spatial_with_target(world, ids, level, None)
    }

    /// Explicit diagnostic request in the actual render destination's coordinates.
    pub fn observe_spatial_for_target(
        &self,
        world: &World,
        ids: &[String],
        level: crate::render_contract::DiagnosticLevel,
        target: &RenderTarget,
    ) -> Option<crate::render_geometry::SpatialObservation> {
        self.observe_spatial_with_target(world, ids, level, Some(target))
    }

    fn observe_spatial_with_target(
        &self,
        world: &World,
        ids: &[String],
        level: crate::render_contract::DiagnosticLevel,
        target: Option<&RenderTarget>,
    ) -> Option<crate::render_geometry::SpatialObservation> {
        if level == crate::render_contract::DiagnosticLevel::Off {
            return None;
        }
        self.geometry_snapshot.as_ref().and_then(|snapshot| {
            self.render_dispatcher
                .with_resources(|manager| {
                    snapshot.observe(
                        self.host_frame,
                        crate::render_geometry::SpatialObservationContext {
                            world,
                            scene: &self.render_scene,
                            target,
                        },
                        manager,
                        ids,
                        level,
                    )
                })
                .ok()
                .flatten()
        })
    }

    /// Engine consumer registration of a secondary camera in the original view owner.
    pub fn project_auxiliary_camera_view(
        &mut self,
        world: &World,
        id: &crate::ids::SourceEntityId,
        viewport: crate::render_state::Viewport,
        kind: crate::render_state::RenderViewKind,
        target: crate::render_state::RenderTargetKind,
    ) -> Result<crate::render_state::RenderViewId, crate::render_contract::ContractError> {
        if world.main_camera() == Some(id) || kind == crate::render_state::RenderViewKind::Game {
            return Err(crate::render_contract::ContractError::new(
                crate::render_contract::ContractErrorCode::InvalidProjection,
                "auxiliaryCamera.mainCollision",
            ));
        }
        if !self.auxiliary_camera_views.contains_key(id) && self.auxiliary_camera_views.len() >= 15
        {
            return Err(crate::render_contract::ContractError::new(
                crate::render_contract::ContractErrorCode::InvalidProjection,
                "view.capacity",
            ));
        }
        let view = self.extract.project_camera_view(
            world,
            &mut self.render_scene,
            id,
            viewport,
            kind.clone(),
            target.clone(),
        )?;
        self.auxiliary_camera_views.insert(
            id.clone(),
            AuxiliaryCameraView {
                view,
                viewport,
                kind,
                target,
            },
        );
        Ok(view)
    }
    pub fn remove_auxiliary_camera_view(&mut self, id: &crate::ids::SourceEntityId) {
        if let Some(request) = self.auxiliary_camera_views.remove(id) {
            if self.geometry_view_targets.remove(&request.view.0).is_some() {
                self.geometry_output_revision = self.geometry_output_revision.saturating_add(1);
            }
            self.render_scene.remove_view(request.view);
        }
    }

    /// Bind a projected camera to an independent output. Changes affect future
    /// packets only; descriptor validation is atomic with production frame building.
    pub fn set_geometry_view_target(
        &mut self,
        view: crate::render_state::RenderViewId,
        target: Option<crate::runtime_renderer::RenderTarget>,
    ) -> Result<(), crate::render_contract::ContractError> {
        if let Some(target) = target {
            if self
                .render_scene
                .view(view)
                .is_none_or(|v| v.spatial.is_none())
            {
                return Err(crate::render_contract::ContractError::new(
                    crate::render_contract::ContractErrorCode::InvalidProjection,
                    "view.outputBindingUnknown",
                ));
            }
            if self.geometry_view_targets.get(&view.0) != Some(&target) {
                self.geometry_output_revision = self
                    .geometry_output_revision
                    .checked_add(1)
                    .ok_or_else(|| {
                        crate::render_contract::ContractError::new(
                            crate::render_contract::ContractErrorCode::InvalidProjection,
                            "view.outputRevisionOverflow",
                        )
                    })?;
                self.geometry_view_targets.insert(view.0, target);
            }
        } else if self.geometry_view_targets.contains_key(&view.0) {
            self.geometry_output_revision = self
                .geometry_output_revision
                .checked_add(1)
                .ok_or_else(|| {
                    crate::render_contract::ContractError::new(
                        crate::render_contract::ContractErrorCode::InvalidProjection,
                        "view.outputRevisionOverflow",
                    )
                })?;
            self.geometry_view_targets.remove(&view.0);
        }
        Ok(())
    }

    /// Immutable production content, shared by retained instances and frame packets.
    pub fn retained_geometry_snapshot(
        &self,
    ) -> Option<&Arc<crate::render_geometry::ProductionGeometrySnapshot>> {
        self.geometry_snapshot.as_ref()
    }

    /// Retry resources of the current committed frame without tick, projection,
    /// observation or camera-history advancement. Existing packets stay immutable.
    pub fn retry_pending_geometry_resources(
        &mut self,
        loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
    ) {
        if self
            .geometry_snapshot
            .as_ref()
            .and_then(|s| s.preparation_error())
            .map(|e| e.code)
            != Some(crate::render_contract::ContractErrorCode::ResourceNotReady)
        {
            return;
        }
        self.geometry_snapshot =
            self.geometry_cache
                .retry_pending_resources(loader)
                .map(|snapshot| {
                    if self.geometry_output_revision == 0 {
                        snapshot
                    } else {
                        Arc::new(snapshot.with_view_targets(
                            &self.geometry_view_targets,
                            self.geometry_output_revision,
                        ))
                    }
                });
    }

    pub fn render_scene(&self) -> &RenderSceneState {
        &self.render_scene
    }

    pub fn set_audio_output(
        &mut self,
        output: Box<dyn crate::runtime_audio::AudioOutput>,
        trace: bool,
    ) {
        self.audio = crate::runtime_audio::RuntimeAudio::new(output);
        self.audio.set_trace_enabled(trace);
    }

    /// Consume committed control media even while a window is minimized and no frame is drawn.
    pub fn flush_control_audio(
        &mut self,
        world: &World,
        loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
    ) {
        self.audio.update(
            world,
            Some(loader),
            std::mem::take(&mut self.pending_audio_commands),
            self.host_frame,
        );
    }

    pub fn set_audio_suspended(&mut self,suspended:bool){self.audio.set_suspended(suspended);}
    pub fn reopen_audio_output(&mut self,world:&World)->Result<(),String>{self.audio.reopen_output(world)}

    pub fn audio_report(&self) -> crate::runtime_audio::RuntimeAudioReport {
        self.audio.report()
    }

    pub fn project_observation_state(&self) -> Option<&ProjectRuntimeObservationState> {
        self.project_observation_state.as_ref()
    }

    pub fn animator2d_module(&self) -> Option<&Animator2DModule> {
        self.frame_loop.animator2d_module()
    }

    pub fn host_control(&self) -> Option<ProjectRuntimeHostControl> {
        self.host_control
    }

    pub fn dispatch_focus_changed(
        &mut self,
        focused: bool,
        world: &mut World,
    ) -> ImmediateAuiActionDispatchOutput {
        if self.focused == Some(focused) {
            return self.dispatch_session_stage(
                ProjectRuntimeSessionStage::AuiActionDispatch,
                &[],
                world,
            );
        }
        self.focused = Some(focused);
        self.dispatch_session_stage(
            ProjectRuntimeSessionStage::FocusChanged { focused },
            &[],
            world,
        )
    }

    pub fn dispatch_aui_actions_immediately(
        &mut self,
        actions: &[AuiAction],
        world: &mut World,
    ) -> ImmediateAuiActionDispatchOutput {
        if self.focused.is_none() {
            self.dispatch_focus_changed(true, world);
        }
        self.dispatch_session_stage(
            ProjectRuntimeSessionStage::AuiActionDispatch,
            actions,
            world,
        )
    }

    fn dispatch_session_stage(
        &mut self,
        stage: ProjectRuntimeSessionStage,
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
                host_control: self.host_control,
                frame_index,
                runtime_advanced: false,
                project_runtime_session_report: Some(report),
                project_observation_state: self.project_observation_state.clone(),
                animator2d_command_count: 0,
                terminal_fault: true,
            };
        }

        if self.host_control.is_some()
            || (actions.is_empty() && stage == ProjectRuntimeSessionStage::AuiActionDispatch)
        {
            return ImmediateAuiActionDispatchOutput {
                host_control: self.host_control,
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
        let stage_report = execute_project_runtime_session_stage_with_media(
            self.project_runtime_session.as_mut(),
            stage,
            frame_index,
            time,
            world,
            actions,
            self.project_runtime_session_report_level,
            &mut animator2d_commands,
            &mut self.pending_audio_commands,
            &mut self.pending_particle_commands,
            &mut self.host_control,
            None,
        );
        let publish_observation = stage == ProjectRuntimeSessionStage::AuiActionDispatch
            || stage_report.status
                == crate::project_runtime_session::ProjectRuntimeSessionStatus::Applied;
        let terminal_fault = stage_report.terminal_fault;
        let mut frame_report = ProjectRuntimeSessionFrameReport::new(
            frame_index,
            self.project_runtime_session.session_id().to_string(),
        );
        frame_report.push_stage(stage_report);
        if terminal_fault {
            self.project_runtime_session_faulted = true;
        } else if let Some(contract) = self
            .project_observation_contract
            .as_ref()
            .filter(|_| publish_observation)
        {
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
            host_control: self.host_control,
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
        self.render_dispatcher
            .submit_frame_output_and_wait(RenderFramePacket {
                frame_identity: self.frame_identity.clone(),
                geometry: self.geometry_snapshot.clone(),
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
            })
            .0
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
        mut runtime_context: Option<RuntimeFrameContext<'_>>,
    ) -> EngineFrameOutput {
        // Prepare clips before gameplay; cleanup and device errors never depend on FixedUpdate.
        self.audio.update(
            world,
            runtime_context
                .as_mut()
                .map(|context| context.instance_loader.asset_loader_mut()),
            Vec::new(),
            self.host_frame,
        );
        let output = self.tick_control_frame(
            input,
            world,
            runtime_context.as_mut().map(|context| RuntimeFrameContext {
                cell_resources: None,
                #[cfg(feature = "real-wgpu")]
                cell_backend: context.cell_backend.as_deref_mut(),
                package: context.package,
                instance_loader: &mut *context.instance_loader,
            }),
        );
        if output.host_control.is_some() || self.project_runtime_session_faulted {
            if let Some(context) = runtime_context.as_mut() {
                context.instance_loader.close_generated_content();
                context.instance_loader.close_project_storage();
                #[cfg(feature = "real-wgpu")]
                if let Some(backend) = context.cell_backend.as_deref_mut() {
                    let _ = self.render_dispatcher.with_resources(|manager| {
                        context.instance_loader.advance_generated_content_native(
                            crate::runtime_instance_loader::GeneratedContentNativeFrame {
                                frame: self.host_frame,
                                world,
                                package: "",
                                cache: &mut self.geometry_cache,
                                manager,
                                backend,
                                physics: self.frame_loop.physics3d_mut(),
                            },
                        );
                    });
                }
            }
        }
        self.audio.update(
            world,
            runtime_context
                .as_mut()
                .map(|context| context.instance_loader.asset_loader_mut()),
            std::mem::take(&mut self.pending_audio_commands),
            self.host_frame,
        );
        output
    }

    fn tick_control_frame(
        &mut self,
        input: EngineFrameInput,
        world: &mut World,
        mut runtime_context: Option<RuntimeFrameContext<'_>>,
    ) -> EngineFrameOutput {
        self.host_frame += 1;
        if let Some(error) = self.render_dispatcher.preparation_error() {
            return self.resource_owner_failure_output(None, error);
        }
        if let Some(context) = runtime_context.as_ref() {
            self.project_observation_contract =
                context.package.manifest.observation_contract.clone();
        }
        let observation_contract = self.project_observation_contract.clone();
        self.reconcile_project_observation_contract(observation_contract.as_ref());
        for focused in &input.focus_changes {
            self.dispatch_focus_changed(*focused, world);
        }
        if self.focused.is_none() {
            self.dispatch_focus_changed(true, world);
        }
        let runtime_advanced = should_advance_runtime(input.mode) && self.host_control.is_none();
        let render_built = should_build_render(input.mode) && input.build_render_submission;
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
                host_control: self.host_control,
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
                host_control: self.host_control,
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
        self.frame_loop.set_spatial_auto_schedule(
            !input.fixed_steps_explicit && input.mode != EngineHostMode::EditorStep,
        );
        let package_identity = runtime_context
            .as_ref()
            .map(|c| {
                format!(
                    "{}:{:?}",
                    c.package.package_dir.display(),
                    c.package.manifest.content_hash
                )
            })
            .unwrap_or_default();
        let world_changed = self
            .frame_source
            .as_ref()
            .is_some_and(|source| source.world_epoch != world.epoch());
        let runtime_frame = self.render_dispatcher.with_resources(|cell_manager| {
            if world_changed {
                self.geometry_cache.reset_world(self.host_frame, cell_manager);
            }
            if let Some(c) = runtime_context.as_mut() {
                #[cfg(feature = "real-wgpu")]
                if let Some(backend) = c.cell_backend.as_deref_mut() {
                    c.instance_loader.advance_generated_content_native(
                        crate::runtime_instance_loader::GeneratedContentNativeFrame {
                            frame: self.host_frame,
                            world,
                            package: &package_identity,
                            cache: &mut self.geometry_cache,
                            manager: cell_manager,
                            backend,
                            physics: self.frame_loop.physics3d_mut(),
                        },
                    );
                } else {
                    c.instance_loader.reject_generated_content_without_native(world);
                }
                #[cfg(not(feature = "real-wgpu"))]
                c.instance_loader.reject_generated_content_without_native(world);
            }
            self.frame_loop
                .tick_runtime_frame_with_project_session_delta_and_fixed_steps(
                    world,
                    &mut self.render_scene,
                    &mut self.extract,
                    input.action_snapshot.as_ref(),
                    input.input_trace_summary,
                    input.unscaled_delta_time,
                    runtime_context.as_mut().map(|c| RuntimeFrameContext {
                        cell_resources: Some(&mut *cell_manager),
                        #[cfg(feature = "real-wgpu")]
                        cell_backend: c.cell_backend.as_deref_mut(),
                        package: c.package,
                        instance_loader: &mut *c.instance_loader,
                    }),
                    ProjectRuntimeFrameSession {
                        session: self.project_runtime_session.as_mut(),
                        actions,
                        report_level: self.project_runtime_session_report_level,
                        observation_contract: self.project_observation_contract.as_ref(),
                    },
                    input.fixed_step_count,
                )
        });
        let runtime_frame = match runtime_frame {
            Ok(frame) => frame,
            Err(error) => return self.resource_owner_failure_output(None, error),
        };
        let mut runtime_frame = match runtime_frame {
            Ok(frame) => frame,
            Err(fault) => {
                self.project_runtime_session_faulted = true;
                return EngineFrameOutput {
                    host_control: self.host_control,
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

        self.host_control = self.host_control.or(runtime_frame.host_control);
        self.pending_audio_commands
            .append(&mut runtime_frame.audio_source_commands);

        self.pending_particle_commands
            .append(&mut runtime_frame.particle_commands);
        self.render_scene.particle_sources = self.particles.update(
            world,
            runtime_context
                .as_mut()
                .map(|c| c.instance_loader.asset_loader_mut()),
            std::mem::take(&mut self.pending_particle_commands),
            self.host_frame,
            runtime_frame.time_trace_summary.delta_time,
        );
        self.render_scene.particle_diagnostics = self.particles.diagnostics.clone();
        self.render_scene.particle_projection_active |=
            !self.render_scene.particle_sources.is_empty();
        self.sync_scene_camera_2d_view(world);
        let mut camera_error = self
            .extract
            .project_main_camera(
                world,
                &mut self.render_scene,
                [
                    self.game_view_target.extent.width,
                    self.game_view_target.extent.height,
                ],
            )
            .err();
        let mut removed_auxiliary = Vec::new();
        for (id, request) in &self.auxiliary_camera_views {
            if let Err(error) = self.extract.project_camera_view(
                world,
                &mut self.render_scene,
                id,
                request.viewport,
                request.kind.clone(),
                request.target.clone(),
            ) {
                if self.geometry_view_targets.remove(&request.view.0).is_some() {
                    self.geometry_output_revision = self.geometry_output_revision.saturating_add(1);
                }
                self.render_scene.remove_view(request.view);
                removed_auxiliary.push(id.clone());
                // Destruction/disable is a normal view removal; other invalid inputs fail closed.
                if world.camera3d(id).is_some_and(|camera| camera.enabled) {
                    camera_error.get_or_insert(error);
                }
            }
        }
        for id in removed_auxiliary {
            self.auxiliary_camera_views.remove(&id);
        }
        let snapshot = self.render_dispatcher.with_resources(|manager| {
            #[cfg(feature = "spine")]
            self.geometry_cache.project_spine(
                self.host_frame,
                world,
                self.frame_loop.spine_module(),
                runtime_context
                    .as_mut()
                    .map(|c| c.instance_loader.asset_loader_mut()),
                manager,
            );
            self.geometry_cache.snapshot(
                self.host_frame,
                &self.render_scene,
                &package_identity,
                runtime_context
                    .as_mut()
                    .map(|c| c.instance_loader.asset_loader_mut()),
                manager,
                camera_error,
            )
        });
        self.geometry_snapshot = Some(match snapshot {
            Ok(snapshot) => snapshot,
            Err(error) => return self.resource_owner_failure_output(Some(runtime_frame), error),
        });

        if world_changed {
            self.geometry_view_targets.clear();
            self.geometry_output_revision = 0;
        }
        if self.geometry_output_revision != 0 {
            self.geometry_snapshot =
                self.geometry_snapshot.as_ref().map(|snapshot| {
                    Arc::new(snapshot.with_view_targets(
                        &self.geometry_view_targets,
                        self.geometry_output_revision,
                    ))
                });
        }
        let package_content_hash = runtime_context
            .as_ref()
            .and_then(|c| c.package.manifest.content_hash.as_deref());
        let session_id = self.project_runtime_session.session_id();
        if self.frame_source.as_ref().is_none_or(|source| {
            source.world_epoch != world.epoch()
                || source.session_id != session_id
                || source.package_content_hash.as_deref() != package_content_hash
        }) {
            self.frame_source = Some(Arc::new(crate::render_thread::RenderFrameSourceIdentity {
                run_id: crate::render_thread::run_identity(self.run_epoch),
                run_epoch: self.run_epoch,
                session_id: session_id.into(),
                package_content_hash: package_content_hash.map(str::to_owned),
                world_epoch: world.epoch(),
            }));
        }
        self.frame_identity = Some(crate::render_thread::RenderFrameIdentity::freeze(
            self.frame_source.as_ref().unwrap().clone(),
            self.host_frame,
            &world.spatial_presentation_delta(),
            self.geometry_snapshot.as_ref().unwrap(),
        ));

        self.output_from_runtime_frame(
            runtime_frame,
            render_built,
            input.aui_overlay,
            input.aui_composition,
            input.runtime_texture_bindings,
        )
    }

    fn resource_owner_failure_output(
        &mut self,
        runtime_frame: Option<RuntimeFrameOutput>,
        diagnostic: RenderThreadDiagnostic,
    ) -> EngineFrameOutput {
        self.host_control = Some(ProjectRuntimeHostControl::Exit);
        let mut output = match runtime_frame {
            Some(frame) => self.output_from_runtime_frame(frame, false, None, None, None),
            None => EngineFrameOutput {
                host_control: self.host_control,
                frame_index: self.host_frame,
                runtime_advanced: false,
                render_built: false,
                runtime_trace: RuntimeTrace::new(),
                render_frame_report: None,
                renderer_feature_frame: None,
                minimal_renderer_frame: None,
                render_thread_frame: None,
                render_submission_report: None,
                render_worker_report: None,
                frame_hash: None,
                time_trace_summary: None,
                project_runtime_session_report: None,
                project_observation_state: self.project_observation_state.clone(),
                animator2d_frame_result: Animator2DFrameResult::default(),
            },
        };
        output.runtime_trace.record(
            self.host_frame,
            "engine.engine_host_loop",
            "FrameFault",
            &diagnostic.code,
            None,
        );
        let (frame, report) = self.render_dispatcher.reject_frame_output(
            RenderFramePacket {
                frame_identity: None,
                geometry: None,
                frame_index: self.host_frame,
                render_scene_state: RenderSceneState::new(),
                render_frame_report: None,
                resource_requests: Vec::new(),
                resource_release_requests: Vec::new(),
                aui_overlay: None,
                aui_composition: None,
                sprite_texture_bindings: None,
                runtime_texture_bindings: None,
                game_view_presentation: None,
                view_id: None,
                quality_profile: QualityProfile::default(),
                render_target: RenderTarget::viewport_texture("failed-render-owner", 1, 1),
            },
            diagnostic,
        );
        output.render_thread_frame = Some(frame);
        output.render_submission_report = Some(report);
        output.render_worker_report = Some(self.render_dispatcher.worker_report());
        output
    }

    fn sync_scene_camera_2d_view(&mut self, world: &World) {
        let Some((camera_id, half_height, camera_position)) = scene_camera_2d(world) else {
            return;
        };
        let mut view = self
            .render_scene
            .views()
            .find(|view| view.view_kind == RenderViewKind::Game && view.spatial.is_none())
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
            self.render_dispatcher
                .submit_frame_output_and_wait(RenderFramePacket {
                    frame_identity: self.frame_identity.clone(),
                    geometry: self.geometry_snapshot.clone(),
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
                })
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
            host_control: self.host_control,
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

fn scene_camera_2d_candidates(world: &World) -> Vec<crate::ids::SourceEntityId> {
    world.query_entities(&crate::query::QuerySpec::all([ComponentTypeId::from(
        "project.camera2d",
    )]))
}

fn scene_camera_2d(world: &World) -> Option<(crate::ids::SourceEntityId, f32, crate::math::Vec3)> {
    let component_type = ComponentTypeId::from("project.camera2d");
    scene_camera_2d_candidates(world)
        .iter()
        .find_map(|entity_id| {
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
    // Match the renderer's default 2D depth span. Identity Z clips world-plane
    // geometry behind a camera at z=10, although the XY-only Sprite path hid it.
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
        -0.001,
        0.0, //
        0.0,
        0.0,
        0.5,
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
    fn scene_camera_2d_candidates_do_not_scan_unrelated_entities() {
        let mut world = World::new();
        for i in 0..128 {
            world.spawn_with_components(
                EntityId::from(format!("unrelated-{i:03}")),
                "Other",
                "mesh",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                None,
                None,
            );
        }
        assert!(scene_camera_2d_candidates(&world).is_empty());
        for (id, kind, enabled, size) in [
            ("camera-a-disabled", "camera", false, 3.0),
            ("camera-b-invalid", "camera", true, -1.0),
            ("camera-c-valid", "camera", true, 4.0),
            ("camera-d-valid", "camera", true, 5.0),
            ("camera-0-noncamera", "mesh", true, 2.0),
        ] {
            let id = EntityId::from(id);
            world.spawn_with_components(
                id.clone(),
                "Camera",
                kind,
                enabled,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                None,
                None,
            );
            world
                .try_insert_dynamic_component(
                    id,
                    ComponentTypeId::from("project.camera2d"),
                    RuntimeValue::object([("orthographicSize", RuntimeValue::F64(size))]),
                )
                .unwrap();
        }
        assert_eq!(scene_camera_2d_candidates(&world).len(), 4);
        let selected = scene_camera_2d(&world).unwrap();
        assert_eq!(selected.0, EntityId::from("camera-c-valid"));
        assert_eq!(selected.1, 4.0);
        assert_eq!(selected.2, Vec3::ZERO);
        world
            .try_insert_dynamic_component(
                EntityId::from("camera-c-valid"),
                ComponentTypeId::from("project.camera2d"),
                RuntimeValue::object([("orthographicSize", RuntimeValue::F64(6.0))]),
            )
            .unwrap();
        assert_eq!(scene_camera_2d(&world).unwrap().1, 6.0);
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
            .find(|view| view.view_kind == RenderViewKind::Game && view.spatial.is_none())
            .expect("scene camera Game view");
        assert_eq!(view.source_entity_id.as_ref(), Some(&camera_id));
        assert_eq!(view.viewport.width, 720);
        assert_eq!(view.viewport.height, 1280);
        assert!((view.projection_matrix[0] - 1.0 / 5.4).abs() < 0.0001);
        assert!((view.projection_matrix[5] - 1.0 / 9.6).abs() < 0.0001);
        assert_eq!(view.view_matrix[12], -2.0);
        assert_eq!(view.view_matrix[13], 3.0);
        // World z=0 must remain inside WGPU's 0..1 clip depth with a camera at z=10.
        // Sprites used only XY; particles consume the complete scene camera matrix.
        let particle_view = crate::particle_render_contract::ParticleRenderView::from_scene_view(
            Some(view),
            720,
            1280,
            [0.0.into(); 3],
        );
        let depth = |z: f32| {
            particle_view.world_to_clip[2][2].to_f32() * z
                + particle_view.world_to_clip[3][2].to_f32()
        };
        for z in [-2.0, 0.0, 2.0] {
            assert!(
                (0.0..=1.0).contains(&depth(z)),
                "world z={z}, clip z={}",
                depth(z)
            );
        }
        assert!(depth(2.0) < depth(0.0) && depth(0.0) < depth(-2.0));
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
            "0.1.0",
        ));
        input.scenes.push(RuntimeScene {
            main_camera: None,
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
                cell_resources: None,
                #[cfg(feature = "real-wgpu")]
                cell_backend: None,
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

    #[test]
    fn project_runtime_initial_lifecycle_animation_survives_registry_initialization() {
        struct InitialAnimation;
        impl ProjectRuntimeSession for InitialAnimation {
            fn session_id(&self) -> &str {
                "initial-animation"
            }
            fn lifecycle(
                &mut self,
                _: ProjectRuntimeSessionContext<'_>,
                _: project_runtime_sdk::ProjectRuntimeLifecycleEvent,
            ) -> ProjectRuntimeSessionOutput {
                let mut mutations = ProjectRuntimeMutationBuffer::new();
                mutations.animator2d_set_trigger(EntityId::from("animated"), "attack");
                ProjectRuntimeSessionOutput::applied(mutations)
            }
            fn handle_aui_actions(
                &mut self,
                _: ProjectRuntimeSessionContext<'_>,
                _: ProjectAuiActionBatch<'_>,
            ) -> ProjectRuntimeSessionOutput {
                ProjectRuntimeSessionOutput::no_op()
            }
            fn fixed_update(
                &mut self,
                _: ProjectRuntimeSessionContext<'_>,
            ) -> ProjectRuntimeSessionOutput {
                ProjectRuntimeSessionOutput::no_op()
            }
        }
        let registry = animator_host_registry();
        let mut world = animator_host_world(&registry.registry_digest);
        let mut host = EngineHostLoop::with_project_runtime_session(
            "initial-animation",
            ProjectLogicRunner::empty(),
            Box::new(InitialAnimation),
        );
        // The real window notifies initial focus before its first frame loads the registry.
        let dispatched = host.dispatch_focus_changed(false, &mut world);
        assert_eq!(dispatched.animator2d_command_count, 1);
        host.set_animator2d_registry(registry).unwrap();
        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );
        assert_eq!(
            world
                .sprite_renderer2d(&EntityId::from("animated"))
                .unwrap()
                .sprite_ref
                .as_deref(),
            Some("attack-0"),
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
        assert!(!output.runtime_trace.events.iter().any(|event| {
            event.system_id.starts_with("project.rule.")
                || event.system_id == "engine.physics2d"
                || event.system_id == "engine.render_extract"
        }));
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
    fn frame_identity_is_frozen_with_external_render_output() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let mut host = EngineHostLoop::new(scene.id);
        host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_deferred_render_submission(),
            &mut world,
        );
        let old = host.render_thread_for_target(RenderTarget::surface("external", 1280, 720));
        let old_plan = serde_json::to_value(&old.renderer_output.rhi_command_plan).unwrap();
        assert!(
            old_plan["frameIdentity"].is_object(),
            "production plan must carry its frozen source, not the latest host state"
        );
        host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_deferred_render_submission(),
            &mut world,
        );
        let new = host.render_thread_for_target(RenderTarget::surface("external", 1280, 720));
        let new_plan = serde_json::to_value(&new.renderer_output.rhi_command_plan).unwrap();
        assert_ne!(
            old_plan["frameIdentity"]["frameIndex"],
            new_plan["frameIdentity"]["frameIndex"]
        );
        assert_eq!(
            old_plan["frameIdentity"]["source"],
            new_plan["frameIdentity"]["source"]
        );
        assert_eq!(
            serde_json::to_value(&old.renderer_output.rhi_command_plan).unwrap(),
            old_plan
        );
    }

    #[test]
    fn exported_game_can_defer_render_submission_to_external_target_owner() {
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene)
            .value
            .expect("world should load");
        mark_renderable_dirty(&mut world);
        let mut host = EngineHostLoop::new(scene.id);

        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_deferred_render_submission(),
            &mut world,
        );

        assert!(output.runtime_advanced);
        assert!(!output.render_built);
        assert!(output.render_thread_frame.is_none());
        assert!(output.render_submission_report.is_none());
        let external = host.render_thread_for_target(RenderTarget::surface("external", 1280, 720));
        assert_eq!(external.report.rhi_status, "ok");
        assert_eq!(external.report.target_id, "external");
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
    fn engine_host_after_worker_timeout_exits_without_waiting_for_the_old_resource_owner() {
        use crate::platform_time::Instant;
        use std::time::Duration;
        let mut host = EngineHostLoop::new_with_render_mode(
            "timed-out-render",
            RenderThreadMode::DedicatedThread,
        );
        let resources = host.render_dispatcher.resource_owner_for_fault_test();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let guard = resources.lock().unwrap();
            ready_tx.send(()).unwrap();
            // The red test itself must remain bounded if Host incorrectly waits.
            let _ = release_rx.recv_timeout(Duration::from_secs(4));
            drop(guard);
        });
        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let first =
            host.render_thread_for_target(RenderTarget::viewport_texture("blocked", 64, 64));
        assert!(first
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == "render_worker_timeout"));
        let scene = renderable_scene_fixture();
        let mut world = load_scene_into_world(&scene).value.unwrap();
        mark_renderable_dirty(&mut world);
        let before = world.spatial_delta();
        let start = Instant::now();
        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::EditorStep),
            &mut world,
        );
        let elapsed = start.elapsed();
        let _ = release_tx.send(());
        holder.join().unwrap();
        assert!(
            elapsed < Duration::from_millis(250),
            "terminal tick waited for failed owner: {elapsed:?}"
        );
        assert_eq!(output.host_control, Some(ProjectRuntimeHostControl::Exit));
        assert!(!output.runtime_advanced && !output.render_built);
        assert!(Arc::ptr_eq(&world.spatial_delta(), &before));
        let report = output.render_submission_report.unwrap();
        assert!(!report.accepted && !report.submitted && !report.presented);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "render_worker_timeout"));
    }

    #[test]
    fn engine_host_failed_resource_owner_exits_before_tick_without_reusing_poisoned_state() {
        for mode in [
            RenderThreadMode::InlineDeterministic,
            RenderThreadMode::DedicatedThread,
        ] {
            let scene = renderable_scene_fixture();
            let mut world = load_scene_into_world(&scene).value.unwrap();
            mark_renderable_dirty(&mut world);
            let before = world.spatial_delta();
            let mut host = EngineHostLoop::new_with_render_mode("failed-render", mode);
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = host
                    .render_dispatcher
                    .with_resources(|_| panic!("injected failed resource owner"));
            }))
            .is_err());
            for _ in 0..3 {
                let output = host.tick(
                    EngineFrameInput::new(EngineHostMode::EditorStep),
                    &mut world,
                );
                assert_eq!(output.host_control, Some(ProjectRuntimeHostControl::Exit));
                assert!(!output.runtime_advanced && !output.render_built);
                assert!(Arc::ptr_eq(&world.spatial_delta(), &before));
                let submission = output.render_submission_report.unwrap();
                assert!(!submission.accepted && !submission.submitted && !submission.presented);
                assert!(submission
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "render_resource_owner_failed"));
            }
        }
    }

    #[test]
    fn engine_host_worker_loss_returns_error_frame_instead_of_panicking_after_flush() {
        let mut host = EngineHostLoop::new_with_render_mode(
            "failed-render",
            RenderThreadMode::DedicatedThread,
        );
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = host
                .render_dispatcher
                .with_resources(|_| panic!("injected original render owner failure"));
        }))
        .is_err());
        let output =
            host.render_thread_for_target(RenderTarget::viewport_texture("failed-target", 64, 64));
        assert_eq!(output.report.rdg_status, "error");
        assert_eq!(output.report.present_status, "not_presented");
        assert!(output
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == "worker_lost"));
        assert!(host.render_dispatcher.worker_report().worker_lost);
        let next =
            host.render_thread_for_target(RenderTarget::viewport_texture("failed-target", 64, 64));
        assert_eq!(next.report.present_status, "not_presented");
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

#[cfg(test)]
mod lifecycle_control_tests {
    use super::*;
    use crate::aui::AuiActionEvent;
    use crate::project_logic::ProjectLogicRunner;
    use crate::project_runtime_session::*;
    use std::sync::{Arc, Mutex};

    struct Session {
        calls: Arc<Mutex<Vec<String>>>,
        status: ProjectRuntimeSessionStatus,
        invalid: bool,
        panic: bool,
        exit_on_fixed: bool,
    }
    impl Session {
        fn exit(&self) -> ProjectRuntimeSessionOutput {
            assert!(!self.panic, "intentional callback panic");
            let mut output =
                ProjectRuntimeSessionOutput::applied(ProjectRuntimeMutationBuffer::new());
            output.status = self.status;
            output.host_control = Some(ProjectRuntimeHostControl::Exit);
            if self.invalid {
                output.mutations.write_transform(
                    crate::ids::EntityId::from("absent"),
                    crate::components::Transform::identity(),
                );
            }
            output
        }
    }
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "lifecycle-test"
        }
        fn lifecycle(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            event: ProjectRuntimeLifecycleEvent,
        ) -> ProjectRuntimeSessionOutput {
            self.calls.lock().unwrap().push(format!("{event:?}"));
            ProjectRuntimeSessionOutput::no_op()
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            self.calls.lock().unwrap().push("action".into());
            self.exit()
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            self.calls.lock().unwrap().push("fixed".into());
            if self.exit_on_fixed {
                self.exit()
            } else {
                ProjectRuntimeSessionOutput::no_op()
            }
        }
    }
    fn make(
        status: ProjectRuntimeSessionStatus,
        invalid: bool,
        panic: bool,
        exit_on_fixed: bool,
    ) -> (EngineHostLoop, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session = Session {
            calls: calls.clone(),
            status,
            invalid,
            panic,
            exit_on_fixed,
        };
        (
            EngineHostLoop::with_project_runtime_session(
                "test",
                ProjectLogicRunner::empty(),
                Box::new(session),
            ),
            calls,
        )
    }
    fn exit_action() -> AuiAction {
        AuiAction {
            action_id: "exit".into(),
            node_id: "exit-button".into(),
            event: AuiActionEvent::Click,
            payload: None,
        }
    }
    #[test]
    fn project_runtime_focus_order_survives_pause_and_zero_steps() {
        let (mut host, calls) = make(ProjectRuntimeSessionStatus::Applied, false, false, false);
        let mut world = World::new();
        host.tick(
            EngineFrameInput::new(EngineHostMode::EditorPause)
                .with_fixed_step_count(0)
                .with_focus_changes(vec![false, false, true]),
            &mut world,
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "FocusChanged { focused: false }",
                "FocusChanged { focused: true }"
            ]
        );
        host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_fixed_step_count(0),
            &mut world,
        );
        assert_eq!(calls.lock().unwrap().len(), 2);
    }
    #[test]
    fn project_runtime_immediate_exit_is_idempotent_with_reports_off() {
        let (mut host, calls) = make(ProjectRuntimeSessionStatus::Applied, false, false, false);
        let mut world = World::new();
        let result = host.dispatch_aui_actions_immediately(&[exit_action()], &mut world);
        assert_eq!(result.host_control, Some(ProjectRuntimeHostControl::Exit));
        assert!(result.project_runtime_session_report.is_none());
        host.dispatch_aui_actions_immediately(&[exit_action()], &mut world);
        let frame = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer),
            &mut world,
        );
        assert!(!frame.runtime_advanced);
        assert_eq!(frame.host_control, Some(ProjectRuntimeHostControl::Exit));
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.as_str() == "action")
                .count(),
            1
        );
        assert!(!calls.lock().unwrap().iter().any(|c| c == "fixed"));
    }
    #[test]
    fn project_runtime_exit_is_dropped_on_failure_noop_and_panic() {
        for (status, invalid, panic) in [
            (ProjectRuntimeSessionStatus::NoOp, false, false),
            (ProjectRuntimeSessionStatus::Rejected, false, false),
            (ProjectRuntimeSessionStatus::Faulted, false, false),
            (ProjectRuntimeSessionStatus::Applied, true, false),
            (ProjectRuntimeSessionStatus::Applied, false, true),
        ] {
            let (mut host, _) = make(status, invalid, panic, false);
            let result = host.dispatch_aui_actions_immediately(&[exit_action()], &mut World::new());
            assert_eq!(result.host_control, None);
        }
    }
    #[test]
    fn project_runtime_fixed_exit_stops_remaining_fixed_steps() {
        let (mut host, calls) = make(ProjectRuntimeSessionStatus::Applied, false, false, true);
        let frame = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_fixed_step_count(4),
            &mut World::new(),
        );
        assert_eq!(frame.host_control, Some(ProjectRuntimeHostControl::Exit));
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.as_str() == "fixed")
                .count(),
            1
        );
    }
    #[test]
    fn project_runtime_action_exit_prevents_same_frame_fixed_update() {
        let (mut host, calls) = make(ProjectRuntimeSessionStatus::Applied, false, false, false);
        let frame = host.tick(
            EngineFrameInput::new(EngineHostMode::HeadlessServer).with_aui_interaction(
                AuiInteractionResult {
                    actions: vec![exit_action()],
                    ..Default::default()
                },
            ),
            &mut World::new(),
        );
        assert_eq!(frame.host_control, Some(ProjectRuntimeHostControl::Exit));
        assert!(!calls.lock().unwrap().iter().any(|c| c == "fixed"));
    }
}
