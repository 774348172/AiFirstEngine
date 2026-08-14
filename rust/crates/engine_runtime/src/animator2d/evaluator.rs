use super::{
    Animator2DDiagnostic, Animator2DParameterKind, Animator2DPlayback, Animator2DTransitionTiming,
    CookedAnimator2DCondition, CookedAnimator2DRegistry, CookedAnimator2DTransition,
    CookedAnimatorController2D, RuntimeAnimator2D,
};
use crate::ids::{EntityId, RuntimeEntityId};
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Animator2DCommand {
    SetBool {
        entity_id: EntityId,
        parameter_id: String,
        value: bool,
    },
    SetTrigger {
        entity_id: EntityId,
        parameter_id: String,
    },
    ResetTrigger {
        entity_id: EntityId,
        parameter_id: String,
    },
}

impl Animator2DCommand {
    pub fn entity_id(&self) -> &EntityId {
        match self {
            Self::SetBool { entity_id, .. }
            | Self::SetTrigger { entity_id, .. }
            | Self::ResetTrigger { entity_id, .. } => entity_id,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Animator2DReportLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Animator2DTraceRecord {
    pub entity_id: String,
    pub state_id: String,
    pub clip_id: String,
    pub frame_index: u32,
    pub sprite_asset_id: String,
    pub transition_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Animator2DFrameResult {
    pub fixed_tick_index: u64,
    pub evaluated_entity_count: usize,
    pub changed_entity_count: usize,
    pub failed_entity_count: usize,
    pub retired_entity_count: usize,
    pub transition_count: usize,
    pub diagnostics: Vec<Animator2DDiagnostic>,
    pub trace: Vec<Animator2DTraceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animator2DEntityState {
    pub state_id: String,
    pub clip_id: String,
    pub frame_index: u32,
    pub completed: bool,
    pub bools: BTreeMap<String, bool>,
    pub triggers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Animator2DInstanceMemory {
    runtime_entity_id: RuntimeEntityId,
    controller_index: u32,
    registry_digest: String,
    state_index: u32,
    frame_index: u32,
    ticks_in_frame: u32,
    speed_accumulator: u32,
    completed: bool,
    bools: Vec<bool>,
    triggers: BTreeSet<u32>,
}

#[derive(Debug, Clone)]
pub struct Animator2DModule {
    registry: CookedAnimator2DRegistry,
    instances: BTreeMap<EntityId, Animator2DInstanceMemory>,
    pending_commands: Vec<Animator2DCommand>,
    emitted_diagnostics: BTreeSet<(String, String, u32)>,
}

impl Animator2DModule {
    pub fn load(registry: CookedAnimator2DRegistry) -> Result<Self, Vec<Animator2DDiagnostic>> {
        registry.validate()?;
        Ok(Self {
            registry,
            instances: BTreeMap::new(),
            pending_commands: Vec::new(),
            emitted_diagnostics: BTreeSet::new(),
        })
    }

    pub fn registry(&self) -> &CookedAnimator2DRegistry {
        &self.registry
    }

    pub fn replace_registry(
        &mut self,
        registry: CookedAnimator2DRegistry,
    ) -> Result<(), Vec<Animator2DDiagnostic>> {
        registry.validate()?;
        if registry.registry_digest != self.registry.registry_digest {
            self.instances.clear();
            self.pending_commands.clear();
            self.emitted_diagnostics.clear();
        }
        self.registry = registry;
        Ok(())
    }

    pub fn apply(&mut self, commands: impl IntoIterator<Item = Animator2DCommand>) {
        self.pending_commands.extend(commands);
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn entity_state(&self, entity_id: &EntityId) -> Option<Animator2DEntityState> {
        let memory = self.instances.get(entity_id)?;
        let controller = self.controller(memory.controller_index)?;
        let state = controller.states.get(memory.state_index as usize)?;
        let clip = self.registry.clips.get(state.clip_index as usize)?;
        Some(Animator2DEntityState {
            state_id: state.id.clone(),
            clip_id: clip.id.clone(),
            frame_index: memory.frame_index,
            completed: memory.completed,
            bools: controller
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.kind == Animator2DParameterKind::Bool)
                .map(|(index, parameter)| {
                    (
                        parameter.id.clone(),
                        memory.bools.get(index).copied().unwrap_or(false),
                    )
                })
                .collect(),
            triggers: memory
                .triggers
                .iter()
                .filter_map(|index| controller.parameters.get(*index as usize))
                .map(|parameter| parameter.id.clone())
                .collect(),
        })
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        fixed_tick_index: u64,
        report_level: Animator2DReportLevel,
    ) -> Animator2DFrameResult {
        let mut result = Animator2DFrameResult {
            fixed_tick_index,
            ..Animator2DFrameResult::default()
        };
        let active_ids = world
            .entity_ids()
            .into_iter()
            .filter(|id| world.animator2d(id).is_some())
            .cloned()
            .collect::<BTreeSet<_>>();
        let before = self.instances.len();
        self.instances
            .retain(|entity_id, _| active_ids.contains(entity_id));
        result.retired_entity_count += before.saturating_sub(self.instances.len());

        let commands = std::mem::take(&mut self.pending_commands);
        let mut commands_by_entity = BTreeMap::<EntityId, Vec<Animator2DCommand>>::new();
        for command in commands {
            commands_by_entity
                .entry(command.entity_id().clone())
                .or_default()
                .push(command);
        }

        for entity_id in active_ids {
            let Some(component) = world.animator2d(&entity_id).cloned() else {
                continue;
            };
            let Some(runtime_entity_id) = world.runtime_id_for_source(&entity_id) else {
                continue;
            };
            if !component.enabled || world.entity(&entity_id).is_some_and(|meta| !meta.enabled) {
                if self.instances.remove(&entity_id).is_some() {
                    result.retired_entity_count += 1;
                }
                continue;
            }
            if world.sprite_renderer2d(&entity_id).is_none() {
                result.failed_entity_count += 1;
                self.record_diagnostic(
                    &mut result,
                    report_level,
                    &entity_id,
                    runtime_entity_id.generation,
                    "animator2d.renderer_missing",
                    "Animator2D requires SpriteRenderer2D on the same entity.",
                );
                continue;
            }
            let Some(controller) = self.valid_controller(&component).cloned() else {
                result.failed_entity_count += 1;
                self.record_diagnostic(
                    &mut result,
                    report_level,
                    &entity_id,
                    runtime_entity_id.generation,
                    "animator2d.controller_missing",
                    "Animator2D controller identity is not present in the loaded registry.",
                );
                continue;
            };
            let reset = self.instances.get(&entity_id).is_none_or(|memory| {
                memory.runtime_entity_id != runtime_entity_id
                    || memory.controller_index != component.controller_index
                    || memory.registry_digest != component.registry_digest
            });
            if reset {
                self.instances.insert(
                    entity_id.clone(),
                    attach_memory(runtime_entity_id, &component, &controller),
                );
            }
            let attached = reset;
            let entity_commands = commands_by_entity.remove(&entity_id).unwrap_or_default();
            let mut command_failed = false;
            {
                let memory = self.instances.get_mut(&entity_id).expect("attached memory");
                for command in entity_commands {
                    if apply_command(memory, &controller, command).is_err() {
                        command_failed = true;
                    }
                }
            }
            if command_failed {
                result.failed_entity_count += 1;
                self.record_diagnostic(
                    &mut result,
                    report_level,
                    &entity_id,
                    runtime_entity_id.generation,
                    "animator2d.command_parameter_invalid",
                    "Animator2D command parameter is missing or has the wrong kind.",
                );
            }

            let registry = &self.registry;
            let memory = self.instances.get_mut(&entity_id).expect("attached memory");
            let immediate =
                select_transition(&controller, memory, Animator2DTransitionTiming::Immediate);
            let mut winning_transition = None;
            if let Some(transition) = immediate {
                winning_transition = Some(transition.id.clone());
                perform_transition(memory, transition);
                result.transition_count += 1;
            } else if !attached {
                let crossed_end = advance_memory(registry, &controller, memory);
                if crossed_end {
                    if let Some(transition) =
                        select_transition(&controller, memory, Animator2DTransitionTiming::ClipEnd)
                    {
                        winning_transition = Some(transition.id.clone());
                        perform_transition(memory, transition);
                        result.transition_count += 1;
                    }
                }
            }

            let Some((state_id, clip_id, frame_index, sprite_asset_id)) =
                present_identity(registry, &controller, memory)
            else {
                result.failed_entity_count += 1;
                continue;
            };
            result.evaluated_entity_count += 1;
            let sprite_changed = world
                .sprite_renderer2d(&entity_id)
                .is_some_and(|sprite| sprite.sprite_ref.as_deref() != Some(&sprite_asset_id));
            if sprite_changed {
                let mut sprite = world
                    .sprite_renderer2d(&entity_id)
                    .expect("renderer was validated")
                    .clone();
                sprite.sprite_ref = Some(sprite_asset_id.clone());
                if world
                    .try_insert_sprite_renderer2d(entity_id.clone(), sprite)
                    .is_ok()
                {
                    result.changed_entity_count += 1;
                }
            }
            if report_level == Animator2DReportLevel::Trace {
                result.trace.push(Animator2DTraceRecord {
                    entity_id: entity_id.to_string(),
                    state_id,
                    clip_id,
                    frame_index,
                    sprite_asset_id,
                    transition_id: winning_transition,
                });
            }
        }

        for (entity_id, _) in commands_by_entity {
            result.failed_entity_count += 1;
            let generation = world
                .runtime_id_for_source(&entity_id)
                .map(|id| id.generation)
                .unwrap_or(u32::MAX);
            self.record_diagnostic(
                &mut result,
                report_level,
                &entity_id,
                generation,
                "animator2d.command_entity_missing",
                "Animator2D command target is missing or has no Animator2D component.",
            );
        }
        result
    }

    fn controller(&self, index: u32) -> Option<&CookedAnimatorController2D> {
        self.registry.controllers.get(index as usize)
    }

    fn valid_controller(
        &self,
        component: &RuntimeAnimator2D,
    ) -> Option<&CookedAnimatorController2D> {
        if component.registry_digest != self.registry.registry_digest {
            return None;
        }
        self.controller(component.controller_index)
            .filter(|controller| controller.id == component.controller_id)
    }

    fn record_diagnostic(
        &mut self,
        result: &mut Animator2DFrameResult,
        report_level: Animator2DReportLevel,
        entity_id: &EntityId,
        generation: u32,
        code: &str,
        message: &str,
    ) {
        if report_level == Animator2DReportLevel::Off {
            return;
        }
        let key = (entity_id.to_string(), code.to_string(), generation);
        if self.emitted_diagnostics.insert(key) {
            result.diagnostics.push(Animator2DDiagnostic::error(
                code,
                format!("entities.{entity_id}"),
                message,
                "Fix the entity Animator2D configuration or command.",
            ));
        }
    }
}

fn attach_memory(
    runtime_entity_id: RuntimeEntityId,
    component: &RuntimeAnimator2D,
    controller: &CookedAnimatorController2D,
) -> Animator2DInstanceMemory {
    let bools = controller
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.kind == Animator2DParameterKind::Bool {
                component
                    .initial_bools
                    .get(&parameter.id)
                    .copied()
                    .unwrap_or(parameter.default_bool)
            } else {
                false
            }
        })
        .collect();
    Animator2DInstanceMemory {
        runtime_entity_id,
        controller_index: component.controller_index,
        registry_digest: component.registry_digest.clone(),
        state_index: controller.entry_state_index,
        frame_index: 0,
        ticks_in_frame: 0,
        speed_accumulator: 0,
        completed: false,
        bools,
        triggers: BTreeSet::new(),
    }
}

fn apply_command(
    memory: &mut Animator2DInstanceMemory,
    controller: &CookedAnimatorController2D,
    command: Animator2DCommand,
) -> Result<(), ()> {
    let (parameter_id, expected_kind) = match &command {
        Animator2DCommand::SetBool { parameter_id, .. } => {
            (parameter_id, Animator2DParameterKind::Bool)
        }
        Animator2DCommand::SetTrigger { parameter_id, .. }
        | Animator2DCommand::ResetTrigger { parameter_id, .. } => {
            (parameter_id, Animator2DParameterKind::Trigger)
        }
    };
    let Some(index) = controller
        .parameters
        .iter()
        .position(|parameter| parameter.id == *parameter_id && parameter.kind == expected_kind)
    else {
        return Err(());
    };
    match command {
        Animator2DCommand::SetBool { value, .. } => memory.bools[index] = value,
        Animator2DCommand::SetTrigger { .. } => {
            memory.triggers.insert(index as u32);
        }
        Animator2DCommand::ResetTrigger { .. } => {
            memory.triggers.remove(&(index as u32));
        }
    }
    Ok(())
}

fn select_transition<'a>(
    controller: &'a CookedAnimatorController2D,
    memory: &Animator2DInstanceMemory,
    timing: Animator2DTransitionTiming,
) -> Option<&'a CookedAnimator2DTransition> {
    controller
        .transitions
        .iter()
        .filter(|transition| {
            transition.from_state_index == memory.state_index
                && transition.timing == timing
                && transition
                    .conditions
                    .iter()
                    .all(|condition| match condition {
                        CookedAnimator2DCondition::BoolEquals {
                            parameter_index,
                            value,
                        } => memory
                            .bools
                            .get(*parameter_index as usize)
                            .is_some_and(|current| current == value),
                        CookedAnimator2DCondition::Triggered { parameter_index } => {
                            memory.triggers.contains(parameter_index)
                        }
                    })
        })
        .min_by(|left, right| match right.priority.cmp(&left.priority) {
            Ordering::Equal => left.id.cmp(&right.id),
            ordering => ordering,
        })
}

fn perform_transition(
    memory: &mut Animator2DInstanceMemory,
    transition: &CookedAnimator2DTransition,
) {
    for condition in &transition.conditions {
        if let CookedAnimator2DCondition::Triggered { parameter_index } = condition {
            memory.triggers.remove(parameter_index);
        }
    }
    memory.state_index = transition.to_state_index;
    memory.frame_index = 0;
    memory.ticks_in_frame = 0;
    memory.speed_accumulator = 0;
    memory.completed = false;
}

fn advance_memory(
    registry: &CookedAnimator2DRegistry,
    controller: &CookedAnimatorController2D,
    memory: &mut Animator2DInstanceMemory,
) -> bool {
    let Some(state) = controller.states.get(memory.state_index as usize) else {
        return false;
    };
    let Some(clip) = registry.clips.get(state.clip_index as usize) else {
        return false;
    };
    if memory.completed && clip.playback == Animator2DPlayback::Once {
        return false;
    }
    memory.speed_accumulator = memory
        .speed_accumulator
        .saturating_add(state.speed_permille);
    let steps = memory.speed_accumulator / 1000;
    memory.speed_accumulator %= 1000;
    let mut crossed_end = false;
    for _ in 0..steps {
        let Some(frame) = clip.frames.get(memory.frame_index as usize) else {
            break;
        };
        memory.ticks_in_frame = memory.ticks_in_frame.saturating_add(1);
        if memory.ticks_in_frame < frame.duration_ticks {
            continue;
        }
        memory.ticks_in_frame = 0;
        if (memory.frame_index as usize) + 1 < clip.frames.len() {
            memory.frame_index += 1;
        } else {
            crossed_end = true;
            match clip.playback {
                Animator2DPlayback::Loop => memory.frame_index = 0,
                Animator2DPlayback::Once => {
                    memory.completed = true;
                    break;
                }
            }
        }
    }
    crossed_end
}

fn present_identity(
    registry: &CookedAnimator2DRegistry,
    controller: &CookedAnimatorController2D,
    memory: &Animator2DInstanceMemory,
) -> Option<(String, String, u32, String)> {
    let state = controller.states.get(memory.state_index as usize)?;
    let clip = registry.clips.get(state.clip_index as usize)?;
    let frame = clip.frames.get(memory.frame_index as usize)?;
    Some((
        state.id.clone(),
        clip.id.clone(),
        memory.frame_index,
        frame.sprite_asset_id.clone(),
    ))
}
