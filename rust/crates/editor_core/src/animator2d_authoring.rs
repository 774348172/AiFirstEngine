use crate::{
    Animator2DAssetCooker, AnimatorController2DAsset, AnimatorController2DConditionAsset,
    AnimatorController2DConditionOperation, AnimatorController2DParameterAsset,
    AnimatorController2DStateAsset, AnimatorController2DTransitionAsset, ProjectWriteScope,
    SpriteAnimationClip2DAsset, SpriteAnimationFrame2DAsset, ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION,
    SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION,
};
use editor_ui_model::{
    Animator2DAuthoringCommand, Animator2DAuthoringDiagnostic, Animator2DAuthoringModel,
    Animator2DAuthoringResult, Animator2DAuthoringStatus, Animator2DClipModel,
    Animator2DComponentModel, Animator2DConditionModel, Animator2DControllerModel,
    Animator2DFrameModel, Animator2DParameterKindModel, Animator2DParameterModel,
    Animator2DPlayObservationModel, Animator2DPlaybackModel, Animator2DPreviewModel,
    Animator2DPreviewRunState, Animator2DRelationshipEdge, Animator2DStateModel,
    Animator2DTransitionModel, Animator2DTransitionTimingModel,
};
use engine_runtime::animator2d::{
    Animator2DCommand, Animator2DDiagnostic, Animator2DFrameResult, Animator2DModule,
    Animator2DParameterKind, Animator2DPlayback, Animator2DReportLevel, Animator2DTransitionTiming,
    CookedAnimator2DRegistry, RuntimeAnimator2D,
};
use engine_runtime::archetype::ComponentValue;
use engine_runtime::components::{ComponentTypeId, Hierarchy, SpriteRenderer2D};
use engine_runtime::ids::EntityId;
use engine_runtime::world::World;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PREVIEW_ENTITY_ID: &str = "animator2d-preview";

#[derive(Debug, Clone)]
enum ActiveAnimator2DAsset {
    Clip { path: PathBuf, asset_id: String },
    Controller { path: PathBuf, asset_id: String },
}

#[derive(Debug, Clone)]
struct Animator2DPreviewSession {
    controller_id: String,
    world: World,
    module: Animator2DModule,
    run_state: Animator2DPreviewRunState,
    fixed_tick_index: u64,
    last_result: Animator2DFrameResult,
}

#[derive(Debug, Clone, Default)]
pub struct Animator2DAuthoringService {
    clips: BTreeMap<String, SpriteAnimationClip2DAsset>,
    controllers: BTreeMap<String, AnimatorController2DAsset>,
    active: Option<ActiveAnimator2DAsset>,
    component: Option<Animator2DComponentModel>,
    dirty: bool,
    diagnostics: Vec<Animator2DAuthoringDiagnostic>,
    preview: Option<Animator2DPreviewSession>,
    play_observations: Vec<Animator2DPlayObservationModel>,
}

impl Animator2DAuthoringService {
    pub fn set_play_observations(&mut self, observations: Vec<Animator2DPlayObservationModel>) {
        self.play_observations = observations;
    }

    pub fn clear_play_observations(&mut self) {
        self.play_observations.clear();
    }
    pub fn execute(&mut self, command: Animator2DAuthoringCommand) -> Animator2DAuthoringResult {
        self.diagnostics.clear();
        let result = self.apply(command);
        if let Err(diagnostic) = result {
            self.diagnostics.push(diagnostic);
            return Animator2DAuthoringResult {
                status: Animator2DAuthoringStatus::Rejected,
                model: self.model(),
            };
        }
        Animator2DAuthoringResult {
            status: Animator2DAuthoringStatus::Applied,
            model: self.model(),
        }
    }

    pub fn model(&self) -> Animator2DAuthoringModel {
        let mut model = Animator2DAuthoringModel {
            dirty: self.dirty,
            component: self.component.clone(),
            diagnostics: self.diagnostics.clone(),
            play_observations: self.play_observations.clone(),
            ..Animator2DAuthoringModel::default()
        };
        match self.active.as_ref() {
            Some(ActiveAnimator2DAsset::Clip { path, asset_id }) => {
                model.clip = self.clips.get(asset_id).map(|clip| clip_model(path, clip));
            }
            Some(ActiveAnimator2DAsset::Controller { path, asset_id }) => {
                model.controller = self
                    .controllers
                    .get(asset_id)
                    .map(|controller| controller_model(path, controller));
            }
            None => {}
        }
        if let Some(controller) = model.controller.as_ref() {
            model.relationship_edges = controller
                .transitions
                .iter()
                .map(|transition| Animator2DRelationshipEdge {
                    transition_id: transition.id.clone(),
                    from_state_id: transition.from.clone(),
                    to_state_id: transition.to.clone(),
                })
                .collect();
        }
        model.preview = self.preview_model();
        model
    }

    pub fn tick_preview(&mut self) -> Animator2DAuthoringResult {
        if self
            .preview
            .as_ref()
            .is_none_or(|preview| preview.run_state != Animator2DPreviewRunState::Playing)
        {
            return Animator2DAuthoringResult {
                status: Animator2DAuthoringStatus::Applied,
                model: self.model(),
            };
        }
        self.step_preview()
    }

    pub fn play_observation(
        entity_id: &EntityId,
        module: &Animator2DModule,
        result: &Animator2DFrameResult,
    ) -> Option<Animator2DPlayObservationModel> {
        let state = module.entity_state(entity_id)?;
        Some(Animator2DPlayObservationModel {
            entity_id: entity_id.to_string(),
            read_only: true,
            state_id: state.state_id,
            clip_id: state.clip_id,
            frame_index: state.frame_index,
            completed: state.completed,
            bools: state.bools,
            triggers: state.triggers.into_iter().collect(),
            recent_diagnostic_codes: result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.path.contains(entity_id.as_str()))
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        })
    }

    fn apply(
        &mut self,
        command: Animator2DAuthoringCommand,
    ) -> Result<(), Animator2DAuthoringDiagnostic> {
        match command {
            Animator2DAuthoringCommand::CreateClip { path, asset_id } => {
                let path = PathBuf::from(path);
                self.clips.insert(
                    asset_id.clone(),
                    SpriteAnimationClip2DAsset {
                        schema_version: SPRITE_ANIMATION_CLIP_2D_SCHEMA_VERSION.to_string(),
                        asset_id: asset_id.clone(),
                        playback: Animator2DPlayback::Loop,
                        frames: Vec::new(),
                    },
                );
                self.active = Some(ActiveAnimator2DAsset::Clip { path, asset_id });
                self.dirty = true;
                self.close_preview();
            }
            Animator2DAuthoringCommand::OpenClip { path } => self.open_clip(Path::new(&path))?,
            Animator2DAuthoringCommand::SetClipPlayback { playback } => {
                self.active_clip_mut()?.playback = runtime_playback(playback);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref,
                duration_ticks,
            } => {
                self.active_clip_mut()?
                    .frames
                    .push(SpriteAnimationFrame2DAsset {
                        sprite_ref,
                        duration_ticks,
                    });
                self.mark_changed();
            }
            Animator2DAuthoringCommand::UpdateClipFrame {
                index,
                sprite_ref,
                duration_ticks,
            } => {
                let frame = self
                    .active_clip_mut()?
                    .frames
                    .get_mut(index)
                    .ok_or_else(|| {
                        diagnostic(
                            "animator2d.authoring.frame_missing",
                            format!("frames[{index}]"),
                            "The selected clip frame does not exist.",
                            "Refresh the Clip Inspector and select an existing frame.",
                        )
                    })?;
                frame.sprite_ref = sprite_ref;
                frame.duration_ticks = duration_ticks;
                self.mark_changed();
            }
            Animator2DAuthoringCommand::MoveClipFrame {
                from_index,
                to_index,
            } => {
                let frames = &mut self.active_clip_mut()?.frames;
                if from_index >= frames.len() || to_index >= frames.len() {
                    return Err(diagnostic(
                        "animator2d.authoring.frame_missing",
                        "frames",
                        "The requested frame reorder is outside the clip frame list.",
                        "Refresh the Clip Inspector and retry with valid frame indices.",
                    ));
                }
                let frame = frames.remove(from_index);
                frames.insert(to_index, frame);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::RemoveClipFrame { index } => {
                let frames = &mut self.active_clip_mut()?.frames;
                if index >= frames.len() {
                    return Err(diagnostic(
                        "animator2d.authoring.frame_missing",
                        format!("frames[{index}]"),
                        "The selected clip frame does not exist.",
                        "Refresh the Clip Inspector and retry.",
                    ));
                }
                frames.remove(index);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::CreateController { path, asset_id } => {
                let path = PathBuf::from(path);
                self.controllers.insert(
                    asset_id.clone(),
                    AnimatorController2DAsset {
                        schema_version: ANIMATOR_CONTROLLER_2D_SCHEMA_VERSION.to_string(),
                        asset_id: asset_id.clone(),
                        parameters: Vec::new(),
                        entry_state_id: String::new(),
                        states: Vec::new(),
                        transitions: Vec::new(),
                    },
                );
                self.active = Some(ActiveAnimator2DAsset::Controller { path, asset_id });
                self.dirty = true;
                self.close_preview();
            }
            Animator2DAuthoringCommand::OpenController { path } => {
                self.open_controller(Path::new(&path))?
            }
            Animator2DAuthoringCommand::UpsertParameter { parameter } => {
                let controller = self.active_controller_mut()?;
                upsert_by_id(&mut controller.parameters, parameter.into(), |value| {
                    &value.id
                });
                controller
                    .parameters
                    .sort_by(|left, right| left.id.cmp(&right.id));
                self.mark_changed();
            }
            Animator2DAuthoringCommand::RemoveParameter { parameter_id } => {
                self.active_controller_mut()?
                    .parameters
                    .retain(|parameter| parameter.id != parameter_id);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::UpsertState { state } => {
                let controller = self.active_controller_mut()?;
                upsert_by_id(&mut controller.states, state.into(), |value| &value.id);
                controller
                    .states
                    .sort_by(|left, right| left.id.cmp(&right.id));
                self.mark_changed();
            }
            Animator2DAuthoringCommand::RemoveState { state_id } => {
                let controller = self.active_controller_mut()?;
                controller.states.retain(|state| state.id != state_id);
                controller
                    .transitions
                    .retain(|transition| transition.from != state_id && transition.to != state_id);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::SetEntryState { state_id } => {
                self.active_controller_mut()?.entry_state_id = state_id;
                self.mark_changed();
            }
            Animator2DAuthoringCommand::UpsertTransition { transition } => {
                let controller = self.active_controller_mut()?;
                upsert_by_id(&mut controller.transitions, transition.into(), |value| {
                    &value.id
                });
                controller
                    .transitions
                    .sort_by(|left, right| left.id.cmp(&right.id));
                self.mark_changed();
            }
            Animator2DAuthoringCommand::RemoveTransition { transition_id } => {
                self.active_controller_mut()?
                    .transitions
                    .retain(|transition| transition.id != transition_id);
                self.mark_changed();
            }
            Animator2DAuthoringCommand::SaveActive => self.save_active()?,
            Animator2DAuthoringCommand::ReloadActive => self.reload_active()?,
            Animator2DAuthoringCommand::SetComponent { component } => {
                self.save_component(&component)?;
                self.component = Some(component);
            }
            Animator2DAuthoringCommand::StartPreview { controller_id } => {
                self.start_preview(&controller_id)?;
            }
            Animator2DAuthoringCommand::PreviewPlay => {
                self.preview_mut()?.run_state = Animator2DPreviewRunState::Playing;
            }
            Animator2DAuthoringCommand::PreviewPause => {
                self.preview_mut()?.run_state = Animator2DPreviewRunState::Paused;
            }
            Animator2DAuthoringCommand::PreviewRestart => {
                let controller_id = self.preview_mut()?.controller_id.clone();
                self.start_preview(&controller_id)?;
            }
            Animator2DAuthoringCommand::PreviewStepTick => self.step_preview_internal()?,
            Animator2DAuthoringCommand::PreviewSetBool {
                parameter_id,
                value,
            } => self.preview_command(Animator2DCommand::SetBool {
                entity_id: EntityId::from(PREVIEW_ENTITY_ID),
                parameter_id,
                value,
            })?,
            Animator2DAuthoringCommand::PreviewSetTrigger { parameter_id } => self
                .preview_command(Animator2DCommand::SetTrigger {
                    entity_id: EntityId::from(PREVIEW_ENTITY_ID),
                    parameter_id,
                })?,
            Animator2DAuthoringCommand::PreviewResetTrigger { parameter_id } => self
                .preview_command(Animator2DCommand::ResetTrigger {
                    entity_id: EntityId::from(PREVIEW_ENTITY_ID),
                    parameter_id,
                })?,
            Animator2DAuthoringCommand::ClosePreview => self.close_preview(),
        }
        Ok(())
    }

    fn active_clip_mut(
        &mut self,
    ) -> Result<&mut SpriteAnimationClip2DAsset, Animator2DAuthoringDiagnostic> {
        let ActiveAnimator2DAsset::Clip { asset_id, .. } =
            self.active.as_ref().ok_or_else(no_active_asset)?
        else {
            return Err(no_active_asset());
        };
        self.clips.get_mut(asset_id).ok_or_else(no_active_asset)
    }

    fn active_controller_mut(
        &mut self,
    ) -> Result<&mut AnimatorController2DAsset, Animator2DAuthoringDiagnostic> {
        let ActiveAnimator2DAsset::Controller { asset_id, .. } =
            self.active.as_ref().ok_or_else(no_active_asset)?
        else {
            return Err(no_active_asset());
        };
        self.controllers
            .get_mut(asset_id)
            .ok_or_else(no_active_asset)
    }

    fn open_clip(&mut self, path: &Path) -> Result<(), Animator2DAuthoringDiagnostic> {
        let text = fs::read_to_string(path).map_err(|error| io_diagnostic(path, error))?;
        let clip = Animator2DAssetCooker::parse_clip_json(&path.display().to_string(), &text)
            .map_err(|failure| diagnostics_to_authoring(failure.diagnostics))?;
        let asset_id = clip.asset_id.clone();
        self.clips.insert(asset_id.clone(), clip);
        self.active = Some(ActiveAnimator2DAsset::Clip {
            path: path.to_path_buf(),
            asset_id,
        });
        self.dirty = false;
        self.close_preview();
        Ok(())
    }

    fn open_controller(&mut self, path: &Path) -> Result<(), Animator2DAuthoringDiagnostic> {
        let text = fs::read_to_string(path).map_err(|error| io_diagnostic(path, error))?;
        let controller =
            Animator2DAssetCooker::parse_controller_json(&path.display().to_string(), &text)
                .map_err(|failure| diagnostics_to_authoring(failure.diagnostics))?;
        let asset_id = controller.asset_id.clone();
        self.controllers.insert(asset_id.clone(), controller);
        self.active = Some(ActiveAnimator2DAsset::Controller {
            path: path.to_path_buf(),
            asset_id,
        });
        self.dirty = false;
        self.close_preview();
        Ok(())
    }

    fn save_active(&mut self) -> Result<(), Animator2DAuthoringDiagnostic> {
        self.cook_registry()?;
        let (path, bytes) = match self.active.as_ref().ok_or_else(no_active_asset)? {
            ActiveAnimator2DAsset::Clip { path, asset_id } => {
                let asset = self.clips.get(asset_id).ok_or_else(no_active_asset)?;
                (path.clone(), serde_json::to_vec_pretty(asset))
            }
            ActiveAnimator2DAsset::Controller { path, asset_id } => {
                let asset = self.controllers.get(asset_id).ok_or_else(no_active_asset)?;
                (path.clone(), serde_json::to_vec_pretty(asset))
            }
        };
        let bytes = bytes.map_err(|error| {
            diagnostic(
                "animator2d.authoring.serialize_failed",
                path.display().to_string(),
                error.to_string(),
                "Fix the active Animator2D asset and retry Save.",
            )
        })?;
        write_atomic_path(&path, &bytes)?;
        self.dirty = false;
        Ok(())
    }

    fn reload_active(&mut self) -> Result<(), Animator2DAuthoringDiagnostic> {
        match self.active.clone().ok_or_else(no_active_asset)? {
            ActiveAnimator2DAsset::Clip { path, .. } => self.open_clip(&path),
            ActiveAnimator2DAsset::Controller { path, .. } => self.open_controller(&path),
        }
    }

    fn save_component(
        &self,
        component: &Animator2DComponentModel,
    ) -> Result<(), Animator2DAuthoringDiagnostic> {
        if !self.controllers.contains_key(&component.controller_ref) {
            return Err(diagnostic(
                "animator2d.authoring.controller_missing",
                "Animator2D.controllerRef",
                format!(
                    "Controller '{}' is not open in the authoring service.",
                    component.controller_ref
                ),
                "Choose an open AnimatorController2D asset from the picker.",
            ));
        }
        let path = Path::new(&component.scene_path);
        let mut document: Value =
            serde_json::from_slice(&fs::read(path).map_err(|error| io_diagnostic(path, error))?)
                .map_err(|error| {
                    diagnostic(
                        "animator2d.authoring.scene_parse_failed",
                        path.display().to_string(),
                        error.to_string(),
                        "Fix the Scene document before editing Animator2D.",
                    )
                })?;
        let entities = document["entities"].as_array_mut().ok_or_else(|| {
            diagnostic(
                "animator2d.authoring.scene_entities_missing",
                "entities",
                "Scene document has no entities array.",
                "Open a valid editor-scene-document.v1 Scene.",
            )
        })?;
        let entity = entities
            .iter_mut()
            .find(|entity| entity["id"] == component.entity_id)
            .ok_or_else(|| {
                diagnostic(
                    "animator2d.authoring.entity_missing",
                    format!("entities.{}", component.entity_id),
                    "The selected Scene entity does not exist.",
                    "Refresh the Hierarchy and select an existing entity.",
                )
            })?;
        let components = entity["components"].as_array_mut().ok_or_else(|| {
            diagnostic(
                "animator2d.authoring.components_missing",
                format!("entities.{}.components", component.entity_id),
                "The selected entity has no component array.",
                "Repair the Scene entity component collection.",
            )
        })?;
        if !components
            .iter()
            .any(|entry| entry["componentType"] == "SpriteRenderer2D")
        {
            return Err(diagnostic(
                "animator2d.renderer_missing",
                format!("entities.{}.components", component.entity_id),
                "Animator2D requires SpriteRenderer2D on the same entity.",
                "Add SpriteRenderer2D before attaching Animator2D.",
            ));
        }
        let value = json!({
            "componentType": "Animator2D",
            "data": {
                "controllerRef": component.controller_ref,
                "enabled": component.enabled,
                "initialBools": component.initial_bools,
            }
        });
        if let Some(existing) = components
            .iter_mut()
            .find(|entry| entry["componentType"] == "Animator2D")
        {
            *existing = value;
        } else {
            components.push(value);
        }
        let bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
            diagnostic(
                "animator2d.authoring.scene_serialize_failed",
                path.display().to_string(),
                error.to_string(),
                "Fix the Scene document and retry.",
            )
        })?;
        write_atomic_path(path, &bytes)
    }

    fn start_preview(&mut self, controller_id: &str) -> Result<(), Animator2DAuthoringDiagnostic> {
        let registry = self.cook_registry()?;
        let controller_index = registry
            .controllers
            .iter()
            .position(|controller| controller.id == controller_id)
            .ok_or_else(|| {
                diagnostic(
                    "animator2d.authoring.controller_missing",
                    "preview.controllerId",
                    format!("Controller '{controller_id}' is not available."),
                    "Open or create the Controller before starting Preview.",
                )
            })? as u32;
        let mut world = World::new();
        let entity_id = EntityId::from(PREVIEW_ENTITY_ID);
        world
            .try_spawn_entity(
                entity_id.clone(),
                "Animator2D Preview",
                "preview",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
            )
            .map_err(world_diagnostic)?;
        world
            .try_insert_sprite_renderer2d(entity_id.clone(), SpriteRenderer2D::default())
            .map_err(world_diagnostic)?;
        world
            .try_insert_component_value(
                entity_id,
                ComponentTypeId::animator2d(),
                ComponentValue::Animator2D(RuntimeAnimator2D {
                    controller_id: controller_id.to_string(),
                    controller_index,
                    registry_digest: registry.registry_digest.clone(),
                    enabled: true,
                    initial_bools: BTreeMap::new(),
                }),
            )
            .map_err(world_diagnostic)?;
        let module = Animator2DModule::load(registry.clone()).map_err(diagnostics_to_authoring)?;
        self.preview = Some(Animator2DPreviewSession {
            controller_id: controller_id.to_string(),
            world,
            module,
            run_state: Animator2DPreviewRunState::Paused,
            fixed_tick_index: 0,
            last_result: Animator2DFrameResult::default(),
        });
        self.step_preview_internal()
    }

    fn step_preview(&mut self) -> Animator2DAuthoringResult {
        let result = self.step_preview_internal();
        if let Err(diagnostic) = result {
            self.diagnostics.push(diagnostic);
            return Animator2DAuthoringResult {
                status: Animator2DAuthoringStatus::Rejected,
                model: self.model(),
            };
        }
        Animator2DAuthoringResult {
            status: Animator2DAuthoringStatus::Applied,
            model: self.model(),
        }
    }

    fn step_preview_internal(&mut self) -> Result<(), Animator2DAuthoringDiagnostic> {
        let preview = self.preview_mut()?;
        preview.fixed_tick_index += 1;
        preview.last_result = preview.module.tick(
            &mut preview.world,
            preview.fixed_tick_index,
            Animator2DReportLevel::Trace,
        );
        Ok(())
    }

    fn preview_command(
        &mut self,
        command: Animator2DCommand,
    ) -> Result<(), Animator2DAuthoringDiagnostic> {
        self.preview_mut()?.module.apply([command]);
        Ok(())
    }

    fn preview_mut(
        &mut self,
    ) -> Result<&mut Animator2DPreviewSession, Animator2DAuthoringDiagnostic> {
        self.preview.as_mut().ok_or_else(|| {
            diagnostic(
                "animator2d.authoring.preview_closed",
                "preview",
                "Animator2D Preview is not open.",
                "Start Preview for an open Controller.",
            )
        })
    }

    fn preview_model(&self) -> Animator2DPreviewModel {
        let Some(preview) = self.preview.as_ref() else {
            return Animator2DPreviewModel::default();
        };
        let state = preview
            .module
            .entity_state(&EntityId::from(PREVIEW_ENTITY_ID));
        let sprite = preview
            .world
            .sprite_renderer2d(&EntityId::from(PREVIEW_ENTITY_ID))
            .and_then(|renderer| renderer.sprite_ref.clone());
        Animator2DPreviewModel {
            run_state: preview.run_state,
            controller_id: Some(preview.controller_id.clone()),
            fixed_tick_index: preview.fixed_tick_index,
            current_state_id: state.as_ref().map(|state| state.state_id.clone()),
            current_clip_id: state.as_ref().map(|state| state.clip_id.clone()),
            current_frame_index: state.as_ref().map(|state| state.frame_index),
            current_sprite_ref: sprite,
            completed: state.as_ref().is_some_and(|state| state.completed),
            bools: state
                .as_ref()
                .map(|state| state.bools.clone())
                .unwrap_or_default(),
            triggers: state
                .as_ref()
                .map(|state| state.triggers.iter().cloned().collect())
                .unwrap_or_default(),
            diagnostics: preview
                .last_result
                .diagnostics
                .iter()
                .cloned()
                .map(authoring_diagnostic)
                .collect(),
        }
    }

    fn close_preview(&mut self) {
        self.preview = None;
    }

    fn mark_changed(&mut self) {
        self.dirty = true;
        self.close_preview();
    }

    fn cook_registry(&self) -> Result<CookedAnimator2DRegistry, Animator2DAuthoringDiagnostic> {
        let sprites = self
            .clips
            .values()
            .flat_map(|clip| clip.frames.iter().map(|frame| frame.sprite_ref.clone()))
            .collect::<BTreeSet<_>>();
        Animator2DAssetCooker::cook(
            self.clips.values().cloned().collect(),
            self.controllers.values().cloned().collect(),
            &sprites,
        )
        .map_err(|failure| diagnostics_to_authoring(failure.diagnostics))
    }
}

impl From<Animator2DParameterModel> for AnimatorController2DParameterAsset {
    fn from(value: Animator2DParameterModel) -> Self {
        Self {
            id: value.id,
            kind: match value.kind {
                Animator2DParameterKindModel::Bool => Animator2DParameterKind::Bool,
                Animator2DParameterKindModel::Trigger => Animator2DParameterKind::Trigger,
            },
            default_bool: value.default_bool,
        }
    }
}

impl From<Animator2DStateModel> for AnimatorController2DStateAsset {
    fn from(value: Animator2DStateModel) -> Self {
        Self {
            id: value.id,
            clip_ref: value.clip_ref,
            speed_permille: value.speed_permille,
        }
    }
}

impl From<Animator2DTransitionModel> for AnimatorController2DTransitionAsset {
    fn from(value: Animator2DTransitionModel) -> Self {
        Self {
            id: value.id,
            from: value.from,
            to: value.to,
            timing: match value.timing {
                Animator2DTransitionTimingModel::Immediate => Animator2DTransitionTiming::Immediate,
                Animator2DTransitionTimingModel::ClipEnd => Animator2DTransitionTiming::ClipEnd,
            },
            priority: value.priority,
            conditions: value.conditions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Animator2DConditionModel> for AnimatorController2DConditionAsset {
    fn from(value: Animator2DConditionModel) -> Self {
        Self {
            parameter: value.parameter,
            operation: if value.triggered {
                AnimatorController2DConditionOperation::Triggered
            } else {
                AnimatorController2DConditionOperation::Equals
            },
            value: value.equals,
        }
    }
}

fn clip_model(path: &Path, clip: &SpriteAnimationClip2DAsset) -> Animator2DClipModel {
    Animator2DClipModel {
        path: path.display().to_string(),
        asset_id: clip.asset_id.clone(),
        playback: model_playback(clip.playback),
        frames: clip
            .frames
            .iter()
            .map(|frame| Animator2DFrameModel {
                sprite_ref: frame.sprite_ref.clone(),
                duration_ticks: frame.duration_ticks,
            })
            .collect(),
    }
}

fn controller_model(
    path: &Path,
    controller: &AnimatorController2DAsset,
) -> Animator2DControllerModel {
    Animator2DControllerModel {
        path: path.display().to_string(),
        asset_id: controller.asset_id.clone(),
        entry_state_id: controller.entry_state_id.clone(),
        parameters: controller
            .parameters
            .iter()
            .map(|parameter| Animator2DParameterModel {
                id: parameter.id.clone(),
                kind: match parameter.kind {
                    Animator2DParameterKind::Bool => Animator2DParameterKindModel::Bool,
                    Animator2DParameterKind::Trigger => Animator2DParameterKindModel::Trigger,
                },
                default_bool: parameter.default_bool,
            })
            .collect(),
        states: controller
            .states
            .iter()
            .map(|state| Animator2DStateModel {
                id: state.id.clone(),
                clip_ref: state.clip_ref.clone(),
                speed_permille: state.speed_permille,
            })
            .collect(),
        transitions: controller
            .transitions
            .iter()
            .map(|transition| Animator2DTransitionModel {
                id: transition.id.clone(),
                from: transition.from.clone(),
                to: transition.to.clone(),
                timing: match transition.timing {
                    Animator2DTransitionTiming::Immediate => {
                        Animator2DTransitionTimingModel::Immediate
                    }
                    Animator2DTransitionTiming::ClipEnd => Animator2DTransitionTimingModel::ClipEnd,
                },
                priority: transition.priority,
                conditions: transition
                    .conditions
                    .iter()
                    .map(|condition| Animator2DConditionModel {
                        parameter: condition.parameter.clone(),
                        equals: condition.value,
                        triggered: condition.operation
                            == AnimatorController2DConditionOperation::Triggered,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn upsert_by_id<T, F>(values: &mut Vec<T>, value: T, id: F)
where
    F: Fn(&T) -> &String,
{
    if let Some(index) = values
        .iter()
        .position(|existing| id(existing) == id(&value))
    {
        values[index] = value;
    } else {
        values.push(value);
    }
}

fn write_atomic_path(path: &Path, bytes: &[u8]) -> Result<(), Animator2DAuthoringDiagnostic> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        diagnostic(
            "animator2d.authoring.path_invalid",
            path.display().to_string(),
            "Animator2D asset path has no file name.",
            "Choose a file inside the project.",
        )
    })?;
    let scope = ProjectWriteScope::open(parent).map_err(|error| {
        diagnostic(
            "animator2d.authoring.scope_open_failed",
            parent.display().to_string(),
            error.to_string(),
            "Choose a writable project path.",
        )
    })?;
    scope
        .write_atomic(Path::new(file_name), bytes)
        .map_err(|error| {
            diagnostic(
                "animator2d.authoring.save_failed",
                path.display().to_string(),
                error.to_string(),
                "Resolve the project write error and retry Save.",
            )
        })?;
    Ok(())
}

fn no_active_asset() -> Animator2DAuthoringDiagnostic {
    diagnostic(
        "animator2d.authoring.asset_not_open",
        "activeAsset",
        "No Animator2D Clip or Controller is open.",
        "Create or open an Animator2D asset first.",
    )
}

fn diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> Animator2DAuthoringDiagnostic {
    Animator2DAuthoringDiagnostic {
        code: code.into(),
        path: Some(path.into()),
        message: message.into(),
        next_action: Some(next_action.into()),
    }
}

fn diagnostics_to_authoring(
    diagnostics: Vec<Animator2DDiagnostic>,
) -> Animator2DAuthoringDiagnostic {
    diagnostics
        .into_iter()
        .next()
        .map(authoring_diagnostic)
        .unwrap_or_else(|| {
            diagnostic(
                "animator2d.authoring.validation_failed",
                "activeAsset",
                "Animator2D validation failed without a diagnostic.",
                "Inspect the active asset and retry.",
            )
        })
}

fn runtime_playback(value: Animator2DPlaybackModel) -> Animator2DPlayback {
    match value {
        Animator2DPlaybackModel::Loop => Animator2DPlayback::Loop,
        Animator2DPlaybackModel::Once => Animator2DPlayback::Once,
    }
}

fn model_playback(value: Animator2DPlayback) -> Animator2DPlaybackModel {
    match value {
        Animator2DPlayback::Loop => Animator2DPlaybackModel::Loop,
        Animator2DPlayback::Once => Animator2DPlaybackModel::Once,
    }
}

fn authoring_diagnostic(value: Animator2DDiagnostic) -> Animator2DAuthoringDiagnostic {
    Animator2DAuthoringDiagnostic {
        code: value.code,
        path: Some(value.path),
        message: value.message,
        next_action: Some(value.next_action),
    }
}

fn io_diagnostic(path: &Path, error: std::io::Error) -> Animator2DAuthoringDiagnostic {
    diagnostic(
        "animator2d.authoring.read_failed",
        path.display().to_string(),
        error.to_string(),
        "Choose an existing readable Animator2D asset.",
    )
}

fn world_diagnostic(
    error: engine_runtime::world::WorldMutationError,
) -> Animator2DAuthoringDiagnostic {
    diagnostic(
        error.code,
        PREVIEW_ENTITY_ID,
        error.message,
        "Restart the isolated Animator2D Preview.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn animator2d_authoring_clip_controller_save_reload_and_stable_order() {
        let root = temp_root("save-reload");
        fs::create_dir_all(&root).unwrap();
        let clip_path = root.join("idle.spriteanim2d.json");
        let controller_path = root.join("main.animator2d.json");
        let mut service = Animator2DAuthoringService::default();

        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateClip {
                path: clip_path.display().to_string(),
                asset_id: "idle".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "idle-b".to_string(),
                duration_ticks: 2,
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "idle-a".to_string(),
                duration_ticks: 1,
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::MoveClipFrame {
                from_index: 1,
                to_index: 0,
            },
        );
        applied(&mut service, Animator2DAuthoringCommand::SaveActive);

        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateController {
                path: controller_path.display().to_string(),
                asset_id: "main".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::UpsertState {
                state: Animator2DStateModel {
                    id: "z-state".to_string(),
                    clip_ref: "idle".to_string(),
                    speed_permille: 1000,
                },
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::UpsertState {
                state: Animator2DStateModel {
                    id: "a-state".to_string(),
                    clip_ref: "idle".to_string(),
                    speed_permille: 1000,
                },
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::SetEntryState {
                state_id: "a-state".to_string(),
            },
        );
        applied(&mut service, Animator2DAuthoringCommand::SaveActive);
        applied(&mut service, Animator2DAuthoringCommand::ReloadActive);

        let model = service.model();
        assert!(!model.dirty);
        assert_eq!(model.controller.unwrap().states[0].id, "a-state");
        let clip: SpriteAnimationClip2DAsset =
            serde_json::from_slice(&fs::read(clip_path).unwrap()).unwrap();
        assert_eq!(clip.frames[0].sprite_ref, "idle-a");
    }

    #[test]
    fn animator2d_authoring_invalid_edit_is_rejected_without_saving() {
        let root = temp_root("invalid");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("bad.spriteanim2d.json");
        let mut service = Animator2DAuthoringService::default();
        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateClip {
                path: path.display().to_string(),
                asset_id: "bad".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "sprite".to_string(),
                duration_ticks: 0,
            },
        );

        let result = service.execute(Animator2DAuthoringCommand::SaveActive);

        assert_eq!(result.status, Animator2DAuthoringStatus::Rejected);
        assert_eq!(
            result.model.diagnostics[0].code,
            "animator2d.frame_duration_invalid"
        );
        assert!(!path.exists());
    }

    #[test]
    fn animator2d_authoring_session_routes_component_inspector_to_atomic_scene_save() {
        let root = temp_root("session-component");
        fs::create_dir_all(&root).unwrap();
        let mut session = crate::EditorSession::new();
        let clip_path = root.join("idle.spriteanim2d.json");
        let controller_path = root.join("main.animator2d.json");
        for command in [
            Animator2DAuthoringCommand::CreateClip {
                path: clip_path.display().to_string(),
                asset_id: "idle".to_string(),
            },
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "idle-0".to_string(),
                duration_ticks: 2,
            },
            Animator2DAuthoringCommand::CreateController {
                path: controller_path.display().to_string(),
                asset_id: "main".to_string(),
            },
            Animator2DAuthoringCommand::UpsertState {
                state: Animator2DStateModel {
                    id: "idle".to_string(),
                    clip_ref: "idle".to_string(),
                    speed_permille: 1000,
                },
            },
            Animator2DAuthoringCommand::SetEntryState {
                state_id: "idle".to_string(),
            },
        ] {
            let result = session.execute_animator2d_authoring_command(command);
            assert_eq!(result.status, Animator2DAuthoringStatus::Applied);
        }
        let scene_path = root.join("main.scene.json");
        fs::write(
            &scene_path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": "editor-scene-document.v1",
                "id": "scene",
                "entities": [{
                    "id": "actor",
                    "components": [{
                        "componentType": "SpriteRenderer2D",
                        "data": {"spriteRef": {"id": "idle-0", "type": "texture"}}
                    }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let result = session.execute_animator2d_authoring_command(
            Animator2DAuthoringCommand::SetComponent {
                component: Animator2DComponentModel {
                    scene_path: scene_path.display().to_string(),
                    entity_id: "actor".to_string(),
                    controller_ref: "main".to_string(),
                    enabled: true,
                    initial_bools: BTreeMap::new(),
                },
            },
        );

        assert_eq!(result.status, Animator2DAuthoringStatus::Applied);
        let scene: Value = serde_json::from_slice(&fs::read(scene_path).unwrap()).unwrap();
        assert_eq!(
            scene["entities"][0]["components"][1]["componentType"],
            "Animator2D"
        );
        assert_eq!(
            scene["entities"][0]["components"][1]["data"]["controllerRef"],
            "main"
        );
        assert!(session.animator2d_authoring_model().component.is_some());
    }

    #[test]
    fn animator2d_preview_uses_runtime_evaluator_and_releases_isolated_memory() {
        let root = temp_root("preview");
        fs::create_dir_all(&root).unwrap();
        let mut service = preview_service(&root);

        applied(
            &mut service,
            Animator2DAuthoringCommand::StartPreview {
                controller_id: "main".to_string(),
            },
        );
        assert_eq!(service.model().preview.fixed_tick_index, 1);
        assert_eq!(
            service.model().preview.current_sprite_ref.as_deref(),
            Some("idle-0")
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::PreviewSetTrigger {
                parameter_id: "attack".to_string(),
            },
        );
        applied(&mut service, Animator2DAuthoringCommand::PreviewStepTick);
        assert_eq!(
            service.model().preview.current_state_id.as_deref(),
            Some("attack")
        );
        applied(&mut service, Animator2DAuthoringCommand::PreviewPause);
        let paused_tick = service.model().preview.fixed_tick_index;
        service.tick_preview();
        assert_eq!(service.model().preview.fixed_tick_index, paused_tick);
        applied(&mut service, Animator2DAuthoringCommand::PreviewRestart);
        assert_eq!(
            service.model().preview.current_state_id.as_deref(),
            Some("idle")
        );
        applied(&mut service, Animator2DAuthoringCommand::ClosePreview);
        assert_eq!(
            service.model().preview.run_state,
            Animator2DPreviewRunState::Closed
        );
    }

    #[test]
    fn animator2d_preview_and_play_observation_are_isolated_and_read_only() {
        let root = temp_root("preview-play-isolation");
        fs::create_dir_all(&root).unwrap();
        let mut service = preview_service(&root);
        applied(
            &mut service,
            Animator2DAuthoringCommand::StartPreview {
                controller_id: "main".to_string(),
            },
        );
        let preview_before = service.model().preview;

        service.set_play_observations(vec![Animator2DPlayObservationModel {
            entity_id: "runtime-actor".to_string(),
            read_only: true,
            state_id: "runtime-state".to_string(),
            clip_id: "runtime-clip".to_string(),
            frame_index: 3,
            completed: false,
            bools: BTreeMap::from([("alert".to_string(), true)]),
            triggers: vec!["attack".to_string()],
            recent_diagnostic_codes: vec!["animator2d.test".to_string()],
        }]);

        let observed = service.model();
        assert_eq!(observed.preview, preview_before);
        assert_eq!(observed.play_observations.len(), 1);
        assert!(observed.play_observations[0].read_only);
        service.clear_play_observations();
        let cleared = service.model();
        assert!(cleared.play_observations.is_empty());
        assert_eq!(cleared.preview, preview_before);
    }

    fn preview_service(root: &Path) -> Animator2DAuthoringService {
        let mut service = Animator2DAuthoringService::default();
        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateClip {
                path: root.join("idle.spriteanim2d.json").display().to_string(),
                asset_id: "idle".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "idle-0".to_string(),
                duration_ticks: 2,
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateClip {
                path: root.join("attack.spriteanim2d.json").display().to_string(),
                asset_id: "attack".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::AddClipFrame {
                sprite_ref: "attack-0".to_string(),
                duration_ticks: 2,
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::CreateController {
                path: root.join("main.animator2d.json").display().to_string(),
                asset_id: "main".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::UpsertParameter {
                parameter: Animator2DParameterModel {
                    id: "attack".to_string(),
                    kind: Animator2DParameterKindModel::Trigger,
                    default_bool: None,
                },
            },
        );
        for (id, clip_ref) in [("idle", "idle"), ("attack", "attack")] {
            applied(
                &mut service,
                Animator2DAuthoringCommand::UpsertState {
                    state: Animator2DStateModel {
                        id: id.to_string(),
                        clip_ref: clip_ref.to_string(),
                        speed_permille: 1000,
                    },
                },
            );
        }
        applied(
            &mut service,
            Animator2DAuthoringCommand::SetEntryState {
                state_id: "idle".to_string(),
            },
        );
        applied(
            &mut service,
            Animator2DAuthoringCommand::UpsertTransition {
                transition: Animator2DTransitionModel {
                    id: "idle-to-attack".to_string(),
                    from: "idle".to_string(),
                    to: "attack".to_string(),
                    timing: Animator2DTransitionTimingModel::Immediate,
                    priority: 10,
                    conditions: vec![Animator2DConditionModel {
                        parameter: "attack".to_string(),
                        equals: None,
                        triggered: true,
                    }],
                },
            },
        );
        service
    }

    fn applied(service: &mut Animator2DAuthoringService, command: Animator2DAuthoringCommand) {
        let result = service.execute(command);
        assert_eq!(
            result.status,
            Animator2DAuthoringStatus::Applied,
            "{:?}",
            result.model.diagnostics
        );
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("animator2d-authoring-{name}-{stamp}"))
    }
}
