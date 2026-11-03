#[cfg_attr(test, path = "project_runtime_session/spatial.rs")]
mod spatial;
#[cfg_attr(test, path = "project_runtime_session/structural.rs")]
mod structural;
#[cfg_attr(test, path = "project_runtime_session/generated_content.rs")]
mod generated_content;
use crate::animator2d::Animator2DCommand;
use crate::archetype::ComponentValue;
use crate::aui::AuiAction;
use crate::canonical_digest::sha256_prefixed;
use crate::component_value::RuntimeValue;
use crate::components::{ComponentTypeId, Transform};
use crate::field_path::FieldPath;
use crate::gameplay_command::GameplayCommand;
use crate::ids::EntityId;
use crate::project_observation::{
    validate_project_observation_values, CookedProjectObservationContract, ProjectObservationValue,
    ProjectRuntimeObservationDiagnostic, ProjectRuntimeObservationState,
};
use crate::runtime_audio::{AudioSourceAction, AudioSourceCommand};
use crate::runtime_particles::{ParticleAction, ParticleCommand};
use crate::runtime_time::TimeContext;
use crate::world::World;
use crate::world_api::{
    prepare_component_field_write, WorldApiError, WorldReadApi, WorldWriteRecord,
};
pub use project_runtime_sdk::{ProjectRuntimeHostControl, ProjectRuntimeLifecycleEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const EMPTY_PROJECT_RUNTIME_SESSION_ID: &str = "engine.empty.runtime.session";

#[derive(Debug, Clone, Copy)]
pub struct ProjectRuntimeSessionCreateContext<'a> {
    pub project_id: &'a str,
    pub module_id: &'a str,
}

pub struct ProjectRuntimeSessionContext<'a> {
    pub frame_index: u64,
    pub time: TimeContext,
    pub world: WorldReadApi<'a>,
}

pub struct ProjectRuntimeObservationContext<'a> {
    pub frame_index: u64,
    pub time: TimeContext,
    pub world: WorldReadApi<'a>,
    pub contract: &'a CookedProjectObservationContract,
    pub report_level: ProjectRuntimeSessionReportLevel,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectRuntimeObservationOutput {
    values: BTreeMap<String, ProjectObservationValue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeAuiExtensionStats {
    pub calls: u64,
    pub describe_nodes: u64,
    pub measure_nodes: u64,
    pub layout_nodes: u64,
    pub paint_nodes: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
}

impl ProjectRuntimeObservationOutput {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        path: impl Into<String>,
        value: ProjectObservationValue,
    ) -> Option<ProjectObservationValue> {
        self.values.insert(path.into(), value)
    }

    pub fn with_value(mut self, path: impl Into<String>, value: ProjectObservationValue) -> Self {
        self.insert(path, value);
        self
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn into_values(self) -> BTreeMap<String, ProjectObservationValue> {
        self.values
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectAuiActionBatch<'a> {
    actions: &'a [AuiAction],
}

impl<'a> ProjectAuiActionBatch<'a> {
    pub fn new(actions: &'a [AuiAction]) -> Self {
        Self { actions }
    }

    pub fn actions(&self) -> &'a [AuiAction] {
        self.actions
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeSessionStatus {
    Applied,
    NoOp,
    Unhandled,
    Rejected,
    Faulted,
}

#[derive(Debug, Clone, PartialEq)]
enum ProjectRuntimeMutationOperation {
    SpatialLocalTransform {
        entity: project_runtime_sdk::spatial_bulk::SpatialEntityKey,
        transform: project_runtime_sdk::ProjectRuntimeTransform3D,
    },
    Cell(project_runtime_sdk::ProjectRuntimeCellIntent),
    InstantiateSpatialPrefab {
        request_id: u64,
        prefab: project_runtime_sdk::ProjectRuntimeSpatialAssetRef,
    },
    DespawnSpatialPrefab {
        entity_id: EntityId,
        generation: u64,
    },
    Spatial {
        entity_id: EntityId,
        generation: u64,
        intent: project_runtime_sdk::ProjectRuntimeSpatialIntent,
    },
    ReplaceComponent {
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    },
    WriteComponentField {
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: FieldPath,
        value: RuntimeValue,
    },
    WriteTransform {
        entity_id: EntityId,
        transform: Transform,
    },
    GameplayCommand(GameplayCommand),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectRuntimeMutationBuffer {
    generated_content: Vec<generated_content::Intent>,
    operations: Vec<ProjectRuntimeMutationOperation>,
    // Pure hot batches stay typed until the original World commit. A cold
    // operation materializes the preceding rows once, preserving total order.
    spatial_transforms: Vec<(
        project_runtime_sdk::spatial_bulk::SpatialEntityKey,
        project_runtime_sdk::ProjectRuntimeTransform3D,
    )>,
    animator2d_commands: Vec<Animator2DCommand>,
    animator_commands: Vec<crate::spine_animation::session::AnimatorCommand>,
    animator_validated: bool,
    audio_source_intents: Vec<(EntityId, AudioSourceAction)>,
    particle_intents: Vec<(EntityId, u64, ParticleAction)>,
}

#[cfg(test)]
std::thread_local! { static EXPANDED_SPATIAL_OPERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

impl ProjectRuntimeMutationBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reserve_spatial_transforms(&mut self, count: usize) {
        self.spatial_transforms.reserve_exact(count);
    }
    fn expand_spatial_transforms(&mut self) {
        if self.spatial_transforms.is_empty() {
            return;
        }
        assert!(
            self.operations.is_empty(),
            "compact rows precede mixed operations"
        );
        self.operations.reserve_exact(self.spatial_transforms.len());
        for (entity, transform) in self.spatial_transforms.drain(..) {
            #[cfg(test)]
            EXPANDED_SPATIAL_OPERATIONS.with(|n| n.set(n.get() + 1));
            self.operations
                .push(ProjectRuntimeMutationOperation::SpatialLocalTransform { entity, transform });
        }
    }
    pub fn cell_command(&mut self, intent: project_runtime_sdk::ProjectRuntimeCellIntent) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::Cell(intent));
    }

    pub fn instantiate_spatial_prefab(
        &mut self,
        request_id: u64,
        prefab: project_runtime_sdk::ProjectRuntimeSpatialAssetRef,
    ) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::InstantiateSpatialPrefab { request_id, prefab });
    }
    pub fn despawn_spatial_prefab(&mut self, entity_id: EntityId, generation: u64) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::DespawnSpatialPrefab {
                entity_id,
                generation,
            });
    }
    pub fn spatial_local_transform(
        &mut self,
        entity: project_runtime_sdk::spatial_bulk::SpatialEntityKey,
        transform: project_runtime_sdk::ProjectRuntimeTransform3D,
    ) {
        if self.operations.is_empty() {
            self.spatial_transforms.push((entity, transform));
            return;
        }
        #[cfg(test)]
        EXPANDED_SPATIAL_OPERATIONS.with(|n| n.set(n.get() + 1));
        self.operations
            .push(ProjectRuntimeMutationOperation::SpatialLocalTransform { entity, transform });
    }

    pub fn spatial_command(
        &mut self,
        entity_id: EntityId,
        generation: u64,
        intent: project_runtime_sdk::ProjectRuntimeSpatialIntent,
    ) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::Spatial {
                entity_id,
                generation,
                intent,
            });
    }

    pub fn is_empty(&self) -> bool {
        self.generated_content.is_empty()
            && self.operations.is_empty()
            && self.spatial_transforms.is_empty()
            && self.animator2d_commands.is_empty()
            && self.animator_commands.is_empty()
            && self.audio_source_intents.is_empty()
            && self.particle_intents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.generated_content.len()
            + self.operations.len()
            + self.spatial_transforms.len()
            + self.animator2d_commands.len()
            + self.animator_commands.len()
            + self.audio_source_intents.len()
            + self.particle_intents.len()
    }

    pub fn replace_component(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    ) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::ReplaceComponent {
                entity_id,
                component_type,
                value,
            });
    }

    pub fn write_component_field(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: FieldPath,
        value: RuntimeValue,
    ) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::WriteComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            });
    }

    pub fn write_transform(&mut self, entity_id: EntityId, transform: Transform) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::WriteTransform {
                entity_id,
                transform,
            });
    }

    pub fn push_gameplay_command(&mut self, command: GameplayCommand) {
        self.expand_spatial_transforms();
        self.operations
            .push(ProjectRuntimeMutationOperation::GameplayCommand(command));
    }

    pub fn animator2d_set_bool(
        &mut self,
        entity_id: EntityId,
        parameter_id: impl Into<String>,
        value: bool,
    ) {
        self.animator2d_commands.push(Animator2DCommand::SetBool {
            entity_id,
            parameter_id: parameter_id.into(),
            value,
        });
    }

    pub fn animator_command(&mut self, command: crate::spine_animation::session::AnimatorCommand) {
        self.animator_commands.push(command);
        self.animator_validated = false;
    }

    pub fn animator2d_command(&mut self, command: Animator2DCommand) {
        self.animator2d_commands.push(command);
    }

    pub fn particle_command(
        &mut self,
        entity_id: EntityId,
        generation: u64,
        action: ParticleAction,
    ) {
        self.particle_intents.push((entity_id, generation, action));
    }

    pub fn audio_source_command(&mut self, entity_id: EntityId, action: AudioSourceAction) {
        self.audio_source_intents.push((entity_id, action));
    }

    pub fn animator2d_set_trigger(&mut self, entity_id: EntityId, parameter_id: impl Into<String>) {
        self.animator2d_commands
            .push(Animator2DCommand::SetTrigger {
                entity_id,
                parameter_id: parameter_id.into(),
            });
    }

    pub fn animator2d_reset_trigger(
        &mut self,
        entity_id: EntityId,
        parameter_id: impl Into<String>,
    ) {
        self.animator2d_commands
            .push(Animator2DCommand::ResetTrigger {
                entity_id,
                parameter_id: parameter_id.into(),
            });
    }

    pub fn prepare(
        self,
        world: &World,
    ) -> Result<PreparedProjectRuntimeMutationBatch, ProjectRuntimeMutationError> {
        self.prepare_with_structure(world)
            .map(|(prepared, _)| prepared)
    }

    // Holds one exclusive World borrow from preparation through application.
    // Nothing may mutate World or invoke a project/loader callback in between.
    fn commit_immediate_for_host(
        self,
        world: &mut World,
    ) -> Result<ProjectRuntimeMutationCommitReport, ProjectRuntimeMutationError> {
        let (prepared, structure) = self.prepare_with_structure(world)?;
        let writes = prepared.component_writes();
        Ok(prepared.apply_validated(world, writes, structure, false))
    }

    fn prepare_with_structure(
        self,
        world: &World,
    ) -> Result<
        (
            PreparedProjectRuntimeMutationBatch,
            crate::world::PreparedSpatialStructure,
        ),
        ProjectRuntimeMutationError,
    > {
        let mut this = self;
        let count = this.len();
        this.expand_spatial_transforms();
        for (index, operation) in this.operations.iter_mut().enumerate() {
            if let ProjectRuntimeMutationOperation::SpatialLocalTransform { entity, transform } =
                operation
            {
                let id = world
                    .resolve_spatial_key(*entity)
                    .map_err(|e| ProjectRuntimeMutationError::from_world_error(index, count, e))?;
                let generation = (u64::from(entity.index) << 32 | u64::from(entity.generation)) + 1;
                *operation = ProjectRuntimeMutationOperation::Spatial {
                    entity_id: id,
                    generation,
                    intent: project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {
                        transform: transform.clone(),
                    },
                };
            }
        }
        this.prepare_normalized(world)
    }

    fn prepare_normalized(
        self,
        world: &World,
    ) -> Result<
        (
            PreparedProjectRuntimeMutationBatch,
            crate::world::PreparedSpatialStructure,
        ),
        ProjectRuntimeMutationError,
    > {
        let staged_count = self.len();
        if !self.generated_content.is_empty() {
            return Err(ProjectRuntimeMutationError::unsupported(0, staged_count));
        }
        if !self.animator_commands.is_empty() && !self.animator_validated {
            return Err(ProjectRuntimeMutationError {
                code: "animator.runtime.registry_uninitialized",
                operation_index: 0,
                message: "Animator commands require the original session host preflight.".into(),
                report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
            });
        }
        for command in &self.animator_commands {
            if command.generation == 0
                || world.entity_generation(&command.entity_id) != Some(command.generation)
            {
                return Err(ProjectRuntimeMutationError {
                    code: "animator.runtime.stale_handle",
                    operation_index: 0,
                    message: "Resolve the current Animator entity incarnation.".into(),
                    report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
                });
            }
        }

        let spatial_count=self.operations.iter().filter(|op|matches!(op,ProjectRuntimeMutationOperation::Spatial{intent,..} if !matches!(intent,project_runtime_sdk::ProjectRuntimeSpatialIntent::Reparent{..}|project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor{..}))).count();
        let structure_count=self.operations.iter().filter(|op|matches!(op,ProjectRuntimeMutationOperation::Spatial{intent,..} if matches!(intent,project_runtime_sdk::ProjectRuntimeSpatialIntent::Reparent{..}|project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor{..}))).count();
        world
            .check_spatial_budget(spatial_count, structure_count)
            .map_err(|e| ProjectRuntimeMutationError::from_world_error(0, staged_count, e))?;
        let mut particle_commands = Vec::new();
        for (entity_id, generation, action) in self.particle_intents {
            let runtime_id = world.runtime_id_for_source(&entity_id);
            if !world
                .entity(&entity_id)
                .is_some_and(|m| m.alive && m.enabled)
                || world.particle_effect(&entity_id).is_none()
                || !runtime_id.is_some_and(|_| {
                    generation == 0 || world.entity_generation(&entity_id) == Some(generation)
                })
            {
                return Err(ProjectRuntimeMutationError {
                    code: "particle_effect.target_unavailable",
                    operation_index: particle_commands.len(),
                    message: format!(
                        "ParticleEffect target {entity_id} is absent, disabled or stale"
                    ),
                    report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
                });
            }
            particle_commands.push(ParticleCommand {
                entity_id,
                runtime_id: runtime_id.unwrap(),
                action,
            });
        }
        let animator2d_commands = self.animator2d_commands;
        let audio_operation_start = self.operations.len() + animator2d_commands.len();
        let mut audio_source_commands = Vec::with_capacity(self.audio_source_intents.len());
        for (index, (entity_id, action)) in self.audio_source_intents.into_iter().enumerate() {
            let live = world
                .entity(&entity_id)
                .is_some_and(|meta| meta.alive && meta.enabled);
            let runtime_id = world.runtime_id_for_source(&entity_id);
            if !live || world.audio_source(&entity_id).is_none() || runtime_id.is_none() {
                return Err(ProjectRuntimeMutationError {
                    code: "audio_source.target_unavailable",
                    operation_index: audio_operation_start + index,
                    message: format!(
                        "AudioSource target {entity_id} must be alive, enabled, and have a typed AudioSource."
                    ),
                    report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
                });
            }
            audio_source_commands.push(AudioSourceCommand {
                entity_id,
                runtime_id: runtime_id.expect("live audio source runtime identity checked"),
                action,
            });
        }
        let mut overlay = BTreeMap::<(EntityId, ComponentTypeId), ComponentValue>::new();
        let mut prepared = Vec::with_capacity(staged_count);
        let mut generations: BTreeMap<_, _> = self
            .animator_commands
            .iter()
            .map(|c| (c.entity_id.clone(), c.generation))
            .collect();
        let mut selected_camera = None;
        let mut history_resets = BTreeMap::new();
        let mut structure_requests = Vec::new();

        for (operation_index, operation) in self.operations.into_iter().enumerate() {
            let prepared_operation = match operation {
                ProjectRuntimeMutationOperation::SpatialLocalTransform { .. } => {
                    unreachable!("normalized typed identity")
                }
                ProjectRuntimeMutationOperation::Spatial {
                    entity_id,
                    generation,
                    intent,
                } => {
                    if generation == 0 || world.entity_generation(&entity_id) != Some(generation) {
                        return Err(ProjectRuntimeMutationError {
                            code: "three_d.stale_entity",
                            operation_index,
                            message: format!("Resolve the current incarnation of {entity_id}"),
                            report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
                        });
                    }
                    generations.insert(entity_id.clone(), generation);
                    if world.transform3d(&entity_id).is_none() {
                        return Err(ProjectRuntimeMutationError {
                            code: "three_d.domain_conflict",
                            operation_index,
                            message: "Spatial target is not World3D".into(),
                            report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
                        });
                    }
                    match &intent {
                        project_runtime_sdk::ProjectRuntimeSpatialIntent::Reparent {
                            parent_entity_id,
                            parent_generation,
                            preserve_world,
                        } => {
                            let parent = parent_entity_id
                                .as_ref()
                                .map(|p| EntityId::from(p.as_str()));
                            if parent.as_ref().is_some_and(|p| {
                                *parent_generation == 0
                                    || world.entity_generation(p) != Some(*parent_generation)
                            }) || (parent.is_none() && *parent_generation != 0)
                            {
                                return Err(ProjectRuntimeMutationError {
                                    code: "three_d.stale_parent",
                                    operation_index,
                                    message: "Resolve a current parent handle".into(),
                                    report: ProjectRuntimeMutationCommitReport::rejected(
                                        staged_count,
                                    ),
                                });
                            }
                            if let Some(parent) = &parent {
                                generations.insert(parent.clone(), *parent_generation);
                            }
                            structure_requests.push(
                                crate::world::SpatialStructureRequest::Reparent {
                                    entity: entity_id,
                                    parent,
                                    preserve_world: *preserve_world,
                                },
                            );
                            continue;
                        }
                        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                            anchor,
                            preserve_world,
                        } => {
                            structure_requests.push(
                                crate::world::SpatialStructureRequest::Anchor {
                                    entity: entity_id,
                                    anchor: *anchor,
                                    preserve_world: *preserve_world,
                                },
                            );
                            continue;
                        }
                        project_runtime_sdk::ProjectRuntimeSpatialIntent::Teleport { .. } => {
                            history_resets.entry(entity_id.clone()).or_insert(None);
                        }
                        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetInterpolation {
                            enabled,
                        } => {
                            history_resets.insert(entity_id.clone(), Some(*enabled));
                            continue;
                        }
                        _ => {}
                    }
                    let value = spatial::prepare_spatial(world, &overlay, &entity_id, intent)
                        .map_err(|e| {
                            ProjectRuntimeMutationError::from_world_error(
                                operation_index,
                                staged_count,
                                e,
                            )
                        })?;
                    let Some(value) = value else {
                        selected_camera = Some(entity_id);
                        continue;
                    };
                    let component_type = value.component_type();
                    overlay.insert((entity_id.clone(), component_type.clone()), value.clone());
                    PreparedProjectRuntimeMutation::WriteComponent {
                        entity_id,
                        component_type,
                        value,
                        field: "*".into(),
                    }
                }
                ProjectRuntimeMutationOperation::ReplaceComponent {
                    entity_id,
                    component_type,
                    value,
                } => {
                    if value.component_type() != component_type {
                        return Err(ProjectRuntimeMutationError::from_world_error(
                            operation_index,
                            staged_count,
                            WorldApiError::invalid_component_value(
                                "prepare_replace_component",
                                entity_id,
                                component_type,
                            ),
                        ));
                    }
                    if current_component(world, &overlay, &entity_id, &component_type).is_none() {
                        return Err(ProjectRuntimeMutationError::from_world_error(
                            operation_index,
                            staged_count,
                            WorldApiError::missing_component(
                                "prepare_replace_component",
                                entity_id,
                                component_type,
                            ),
                        ));
                    }
                    overlay.insert((entity_id.clone(), component_type.clone()), value.clone());
                    PreparedProjectRuntimeMutation::WriteComponent {
                        entity_id,
                        component_type,
                        value,
                        field: "*".to_string(),
                    }
                }
                ProjectRuntimeMutationOperation::WriteComponentField {
                    entity_id,
                    component_type,
                    field_path,
                    value,
                } => {
                    let Some(component) =
                        current_component(world, &overlay, &entity_id, &component_type)
                    else {
                        return Err(ProjectRuntimeMutationError::from_world_error(
                            operation_index,
                            staged_count,
                            WorldApiError::missing_component(
                                "prepare_component_field_write",
                                entity_id,
                                component_type,
                            ),
                        ));
                    };
                    let value = prepare_component_field_write(
                        &entity_id,
                        &component_type,
                        component,
                        &field_path,
                        value,
                    )
                    .map_err(|error| {
                        ProjectRuntimeMutationError::from_world_error(
                            operation_index,
                            staged_count,
                            error,
                        )
                    })?;
                    overlay.insert((entity_id.clone(), component_type.clone()), value.clone());
                    PreparedProjectRuntimeMutation::WriteComponent {
                        entity_id,
                        component_type,
                        value,
                        field: field_path.as_str().to_string(),
                    }
                }
                ProjectRuntimeMutationOperation::WriteTransform {
                    entity_id,
                    transform,
                } => {
                    let component_type = ComponentTypeId::transform();
                    if current_component(world, &overlay, &entity_id, &component_type).is_none() {
                        return Err(ProjectRuntimeMutationError::from_world_error(
                            operation_index,
                            staged_count,
                            WorldApiError::missing_component(
                                "prepare_transform_write",
                                entity_id,
                                component_type,
                            ),
                        ));
                    }
                    let value = ComponentValue::Transform(transform);
                    overlay.insert((entity_id.clone(), component_type.clone()), value.clone());
                    PreparedProjectRuntimeMutation::WriteComponent {
                        entity_id,
                        component_type,
                        value,
                        field: "*".to_string(),
                    }
                }
                ProjectRuntimeMutationOperation::Cell(_)
                | ProjectRuntimeMutationOperation::GameplayCommand(_)
                | ProjectRuntimeMutationOperation::InstantiateSpatialPrefab { .. }
                | ProjectRuntimeMutationOperation::DespawnSpatialPrefab { .. } => {
                    return Err(ProjectRuntimeMutationError::unsupported(
                        operation_index,
                        staged_count,
                    ));
                }
            };
            let PreparedProjectRuntimeMutation::WriteComponent { ref entity_id, .. } =
                prepared_operation;
            if let Some(generation) = world.entity_generation(entity_id) {
                generations.insert(entity_id.clone(), generation);
            }
            prepared.push(prepared_operation);
        }

        let writes = prepared
            .iter()
            .map(|p| match p {
                PreparedProjectRuntimeMutation::WriteComponent {
                    entity_id, value, ..
                } => (entity_id.clone(), value.clone()),
            })
            .collect::<Vec<_>>();
        let structure = world
            .prepare_spatial_structure(&writes, &structure_requests)
            .map_err(|e| ProjectRuntimeMutationError::from_world_error(0, staged_count, e))?;
        let mut combined = writes;
        combined.extend(structure.writes.clone());
        world
            .validate_spatial_structure(&combined, selected_camera.as_ref(), &structure)
            .map_err(|e| ProjectRuntimeMutationError::from_world_error(0, staged_count, e))?;
        Ok((
            PreparedProjectRuntimeMutationBatch {
                world_epoch: world.epoch(),
                generations,
                selected_camera,
                operations: prepared,
                animator2d_commands,
                animator_commands: self.animator_commands,
                audio_source_commands,
                particle_commands,
                staged_count,
                spatial_count,
                history_resets,
                structure_requests,
            },
            structure,
        ))
    }
}

fn current_component(
    world: &World,
    overlay: &BTreeMap<(EntityId, ComponentTypeId), ComponentValue>,
    entity_id: &EntityId,
    component_type: &ComponentTypeId,
) -> Option<ComponentValue> {
    overlay
        .get(&(entity_id.clone(), component_type.clone()))
        .cloned()
        .or_else(|| world.component_value(entity_id, component_type))
}

#[derive(Debug, Clone, PartialEq)]
enum PreparedProjectRuntimeMutation {
    WriteComponent {
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
        field: String,
    },
}

#[derive(Debug)]
pub struct PreparedProjectRuntimeMutationBatch {
    world_epoch: u64,
    generations: BTreeMap<EntityId, u64>,
    selected_camera: Option<EntityId>,
    operations: Vec<PreparedProjectRuntimeMutation>,
    animator2d_commands: Vec<Animator2DCommand>,
    animator_commands: Vec<crate::spine_animation::session::AnimatorCommand>,
    audio_source_commands: Vec<AudioSourceCommand>,
    particle_commands: Vec<ParticleCommand>,
    staged_count: usize,
    spatial_count: usize,
    history_resets: BTreeMap<EntityId, Option<bool>>,
    structure_requests: Vec<crate::world::SpatialStructureRequest>,
}

impl PreparedProjectRuntimeMutationBatch {
    pub fn len(&self) -> usize {
        self.staged_count
    }

    pub fn is_empty(&self) -> bool {
        self.staged_count == 0
    }

    pub fn commit(
        self,
        world: &mut World,
    ) -> Result<ProjectRuntimeMutationCommitReport, ProjectRuntimeMutationError> {
        self.commit_inner(world, true)
    }

    // The session host consumes counts/commands/receipts, never WorldWriteRecord history.
    // Share every preflight and commit check with the public recording path.
    fn commit_for_host(
        self,
        world: &mut World,
    ) -> Result<ProjectRuntimeMutationCommitReport, ProjectRuntimeMutationError> {
        self.commit_inner(world, false)
    }

    fn commit_inner(
        self,
        world: &mut World,
        record_writes: bool,
    ) -> Result<ProjectRuntimeMutationCommitReport, ProjectRuntimeMutationError> {
        if self.world_epoch != world.epoch() {
            return Err(ProjectRuntimeMutationError {
                code: "three_d.stale_entity",
                operation_index: 0,
                message: "World changed after preparation".into(),
                report: ProjectRuntimeMutationCommitReport::rejected(self.staged_count),
            });
        }
        world
            .check_spatial_budget(self.spatial_count, self.structure_requests.len())
            .map_err(|e| ProjectRuntimeMutationError::commit_failed(0, self.staged_count, 0, e))?;
        for (id, generation) in &self.generations {
            if world.entity_generation(id) != Some(*generation) {
                return Err(ProjectRuntimeMutationError {
                    code: "three_d.stale_entity",
                    operation_index: 0,
                    message: format!("Entity {id} changed after preparation"),
                    report: ProjectRuntimeMutationCommitReport::rejected(self.staged_count),
                });
            }
        }
        let writes = self.component_writes();
        let structure = world
            .prepare_spatial_structure(&writes, &self.structure_requests)
            .map_err(|e| ProjectRuntimeMutationError::commit_failed(0, self.staged_count, 0, e))?;
        let mut combined = writes.clone();
        combined.extend(structure.writes.clone());
        world
            .validate_spatial_structure(&combined, self.selected_camera.as_ref(), &structure)
            .map_err(|e| ProjectRuntimeMutationError::commit_failed(0, self.staged_count, 0, e))?;
        Ok(self.apply_validated(world, writes, structure, record_writes))
    }

    fn component_writes(&self) -> Vec<(EntityId, ComponentValue)> {
        self.operations
            .iter()
            .map(|p| match p {
                PreparedProjectRuntimeMutation::WriteComponent {
                    entity_id, value, ..
                } => (entity_id.clone(), value.clone()),
            })
            .collect::<Vec<_>>()
    }

    fn apply_validated(
        self,
        world: &mut World,
        writes: Vec<(EntityId, ComponentValue)>,
        structure: crate::world::PreparedSpatialStructure,
        record_writes: bool,
    ) -> ProjectRuntimeMutationCommitReport {
        let records = if record_writes {
            let mut record_overlay = BTreeMap::new();
            self.operations
                .iter()
                .map(|p| match p {
                    PreparedProjectRuntimeMutation::WriteComponent {
                        entity_id,
                        component_type,
                        value,
                        field,
                    } => {
                        #[cfg(test)]
                        mutation_record_work::RECORDS.with(|count| count.set(count.get() + 1));
                        let before =
                            current_component(world, &record_overlay, entity_id, component_type);
                        record_overlay
                            .insert((entity_id.clone(), component_type.clone()), value.clone());
                        WorldWriteRecord {
                            entity_id: entity_id.clone(),
                            component_type: component_type.clone(),
                            field: field.clone(),
                            before: before.map(|v| format!("{v:?}")),
                            after: Some(format!("{value:?}")),
                        }
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        world.charge_spatial_budget(self.spatial_count, self.structure_requests.len());
        world.commit_deferred_components(writes, self.selected_camera);
        world.commit_spatial_structure(structure);
        if !self.history_resets.is_empty() {
            world.finish_spatial_tick();
        }
        for (id, mode) in self.history_resets {
            world.reset_spatial_history(&id, mode);
        }
        ProjectRuntimeMutationCommitReport {
            generated_content_errors: Vec::new(),
            staged_count: self.staged_count,
            committed_count: self.staged_count,
            rejected_count: 0,
            records,
            animator2d_commands: self.animator2d_commands,
            animator_commands: self.animator_commands,
            audio_source_commands: self.audio_source_commands,
            particle_commands: self.particle_commands,
        }
    }
}

#[cfg(test)]
mod mutation_record_work {
    std::thread_local! {
        pub(super) static RECORDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeMutationError {
    pub code: &'static str,
    pub operation_index: usize,
    pub message: String,
    pub report: ProjectRuntimeMutationCommitReport,
}

impl ProjectRuntimeMutationError {
    fn from_world_error(operation_index: usize, staged_count: usize, error: WorldApiError) -> Self {
        Self {
            code: "project_runtime.mutation_preflight_failed",
            operation_index,
            message: format!("{}: {}", error.code, error.message),
            report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
        }
    }

    fn unsupported(operation_index: usize, staged_count: usize) -> Self {
        Self {
            code: "project_runtime.mutation_unsupported",
            operation_index,
            message:
                "GameplayCommand structural mutation is unsupported in ProjectRuntimeSession v1."
                    .to_string(),
            report: ProjectRuntimeMutationCommitReport::rejected(staged_count),
        }
    }

    fn commit_failed(
        operation_index: usize,
        staged_count: usize,
        committed_count: usize,
        error: WorldApiError,
    ) -> Self {
        Self {
            code: "project_runtime.mutation_commit_failed",
            operation_index,
            message: format!("{}: {}", error.code, error.message),
            report: ProjectRuntimeMutationCommitReport {
                generated_content_errors: Vec::new(),
                staged_count,
                committed_count,
                rejected_count: staged_count.saturating_sub(committed_count),
                records: Vec::new(),
                animator2d_commands: Vec::new(),
                animator_commands: Vec::new(),
                audio_source_commands: Vec::new(),
                particle_commands: Vec::new(),
            },
        }
    }
}

impl fmt::Display for ProjectRuntimeMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at operation {}: {}",
            self.code, self.operation_index, self.message
        )
    }
}

impl std::error::Error for ProjectRuntimeMutationError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectRuntimeMutationCommitReport {
    pub generated_content_errors: Vec<project_runtime_sdk::generated_content::ContentError>,
    pub staged_count: usize,
    pub committed_count: usize,
    pub rejected_count: usize,
    pub records: Vec<WorldWriteRecord>,
    pub animator2d_commands: Vec<Animator2DCommand>,
    pub animator_commands: Vec<crate::spine_animation::session::AnimatorCommand>,
    pub audio_source_commands: Vec<AudioSourceCommand>,
    pub particle_commands: Vec<ParticleCommand>,
}

impl ProjectRuntimeMutationCommitReport {
    fn rejected(staged_count: usize) -> Self {
        Self {
            generated_content_errors: Vec::new(),
            staged_count,
            committed_count: 0,
            rejected_count: staged_count,
            records: Vec::new(),
            animator2d_commands: Vec::new(),
            animator_commands: Vec::new(),
            audio_source_commands: Vec::new(),
            particle_commands: Vec::new(),
        }
    }
}

pub enum ProjectRuntimeMutationPreparation {
    Prepared(PreparedProjectRuntimeMutationBatch),
    Dropped(ProjectRuntimeMutationCommitReport),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProjectRuntimeSessionReportLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeSessionStage {
    AuiActionDispatch,
    FixedUpdate,
    FocusChanged { focused: bool },
}

impl ProjectRuntimeSessionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuiActionDispatch => "aui_action_dispatch",
            Self::FixedUpdate => "fixed_update",
            Self::FocusChanged { .. } => "focus_changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeSessionActionTrace {
    pub batch_index: usize,
    pub action_id: String,
    pub node_id: String,
    pub event: String,
    pub payload_present: bool,
    pub payload_byte_length: usize,
    pub payload_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeSessionStageReport {
    pub stage: ProjectRuntimeSessionStage,
    pub status: ProjectRuntimeSessionStatus,
    pub action_count: usize,
    pub handled_action_count: usize,
    pub unhandled_action_count: usize,
    pub rejected_action_count: usize,
    pub staged_mutation_count: usize,
    pub committed_mutation_count: usize,
    pub rejected_mutation_count: usize,
    pub diagnostics: Vec<String>,
    pub action_trace: Vec<ProjectRuntimeSessionActionTrace>,
    pub terminal_fault: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeSessionFrameReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub session_id: String,
    pub status: String,
    pub discarded_action_count: usize,
    pub stages: Vec<ProjectRuntimeSessionStageReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<ProjectRuntimeObservationFrameReport>,
    pub terminal_fault: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeObservationFrameReport {
    pub status: String,
    pub value_count: usize,
    pub diagnostics: Vec<String>,
}

impl ProjectRuntimeObservationFrameReport {
    fn from_state(state: &ProjectRuntimeObservationState) -> Self {
        match state {
            ProjectRuntimeObservationState::NotProducedYet { .. } => Self {
                status: "not_produced_yet".to_string(),
                value_count: 0,
                diagnostics: Vec::new(),
            },
            ProjectRuntimeObservationState::Published { snapshot } => Self {
                status: "published".to_string(),
                value_count: snapshot.values.len(),
                diagnostics: Vec::new(),
            },
            ProjectRuntimeObservationState::ContractViolated { diagnostics, .. } => Self {
                status: "contract_violated".to_string(),
                value_count: 0,
                diagnostics: diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect(),
            },
        }
    }
}

impl ProjectRuntimeSessionFrameReport {
    pub fn new(frame_index: u64, session_id: impl Into<String>) -> Self {
        Self {
            schema_version: "project-runtime-session-report.v1".to_string(),
            frame_index,
            session_id: session_id.into(),
            status: "advanced".to_string(),
            discarded_action_count: 0,
            stages: Vec::new(),
            observation: None,
            terminal_fault: false,
        }
    }

    pub fn discarded_non_advancing(
        frame_index: u64,
        session_id: impl Into<String>,
        action_count: usize,
    ) -> Self {
        Self {
            schema_version: "project-runtime-session-report.v1".to_string(),
            frame_index,
            session_id: session_id.into(),
            status: "discarded_non_advancing_mode".to_string(),
            discarded_action_count: action_count,
            stages: Vec::new(),
            observation: None,
            terminal_fault: false,
        }
    }

    pub fn reentry_after_fault(frame_index: u64, session_id: impl Into<String>) -> Self {
        Self {
            schema_version: "project-runtime-session-report.v1".to_string(),
            frame_index,
            session_id: session_id.into(),
            status: "faulted".to_string(),
            discarded_action_count: 0,
            stages: vec![ProjectRuntimeSessionStageReport {
                stage: ProjectRuntimeSessionStage::AuiActionDispatch,
                status: ProjectRuntimeSessionStatus::Faulted,
                action_count: 0,
                handled_action_count: 0,
                unhandled_action_count: 0,
                rejected_action_count: 0,
                staged_mutation_count: 0,
                committed_mutation_count: 0,
                rejected_mutation_count: 0,
                diagnostics: vec!["project_runtime.session_reentry_after_fault".to_string()],
                action_trace: Vec::new(),
                terminal_fault: true,
            }],
            observation: None,
            terminal_fault: true,
        }
    }

    pub fn push_stage(&mut self, report: ProjectRuntimeSessionStageReport) {
        if report.terminal_fault {
            self.status = "faulted".to_string();
            self.terminal_fault = true;
        }
        self.stages.push(report);
    }

    pub fn set_observation(&mut self, state: &ProjectRuntimeObservationState) {
        self.observation = Some(ProjectRuntimeObservationFrameReport::from_state(state));
    }
}

pub(crate) fn execute_project_runtime_observation(
    session: &dyn ProjectRuntimeSession,
    frame_index: u64,
    time: TimeContext,
    world: &World,
    contract: &CookedProjectObservationContract,
    report_level: ProjectRuntimeSessionReportLevel,
) -> ProjectRuntimeObservationState {
    let session_id = session.session_id().to_string();
    let callback = catch_unwind(AssertUnwindSafe(|| {
        session.observe(ProjectRuntimeObservationContext {
            frame_index,
            time,
            world: WorldReadApi::new(world),
            contract,
            report_level,
        })
    }));
    let output = match callback {
        Ok(output) => output,
        Err(_) => {
            return ProjectRuntimeObservationState::ContractViolated {
                runtime_frame: frame_index,
                session_id,
                contract_id: contract.contract_id.clone(),
                contract_digest: contract.contract_digest.clone(),
                declared_types: contract
                    .observations
                    .iter()
                    .map(|entry| (entry.path.clone(), entry.value_type))
                    .collect(),
                diagnostics: vec![ProjectRuntimeObservationDiagnostic {
                    code: "project_observation.observe_panicked".to_string(),
                    path: None,
                    message: "Project runtime observation callback panicked.".to_string(),
                }],
            };
        }
    };
    validate_project_observation_values(contract, frame_index, &session_id, output.into_values())
}

pub(crate) fn execute_project_runtime_session_stage_with_media(
    session: &mut dyn ProjectRuntimeSession,
    stage: ProjectRuntimeSessionStage,
    frame_index: u64,
    time: TimeContext,
    world: &mut World,
    actions: &[AuiAction],
    report_level: ProjectRuntimeSessionReportLevel,
    animator2d_commands: &mut Vec<Animator2DCommand>,
    audio_source_commands: &mut Vec<AudioSourceCommand>,
    particle_commands: &mut Vec<ParticleCommand>,
    host_control: &mut Option<ProjectRuntimeHostControl>,
    runtime_context: Option<crate::gameplay_command::RuntimeCommandContext<'_>>,
) -> ProjectRuntimeSessionStageReport {
    execute_project_runtime_session_stage_with_spine(
        session,
        stage,
        frame_index,
        time,
        world,
        actions,
        report_level,
        animator2d_commands,
        audio_source_commands,
        particle_commands,
        host_control,
        runtime_context,
        None,
        None,
    )
}

#[cfg(test)]
pub(crate) mod report_work {
    thread_local! {
        pub(crate) static PAYLOAD_BYTES_HASHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        pub(crate) static CONTENT_USAGE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
}

// Preserve the existing media-stage call boundary and its separate owner borrows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_project_runtime_session_stage_with_spine(
    session: &mut dyn ProjectRuntimeSession,
    stage: ProjectRuntimeSessionStage,
    frame_index: u64,
    time: TimeContext,
    world: &mut World,
    actions: &[AuiAction],
    report_level: ProjectRuntimeSessionReportLevel,
    animator2d_commands: &mut Vec<Animator2DCommand>,
    audio_source_commands: &mut Vec<AudioSourceCommand>,
    particle_commands: &mut Vec<ParticleCommand>,
    host_control: &mut Option<ProjectRuntimeHostControl>,
    runtime_context: Option<crate::gameplay_command::RuntimeCommandContext<'_>>,
    mut spine: Option<&mut crate::spine_animation::session::SpineModule>,
    physics: Option<&crate::physics_3d::PhysicsModule>,
) -> ProjectRuntimeSessionStageReport {
    let action_trace = if report_level == ProjectRuntimeSessionReportLevel::Trace {
        actions
            .iter()
            .enumerate()
            .map(|(batch_index, action)| {
                let payload = action.payload.as_deref();
                ProjectRuntimeSessionActionTrace {
                    batch_index,
                    action_id: action.action_id.clone(),
                    node_id: action.node_id.clone(),
                    event: format!("{:?}", action.event),
                    payload_present: payload.is_some(),
                    payload_byte_length: payload.map(str::len).unwrap_or(0),
                    payload_digest: payload.map(|value| {
                        #[cfg(test)]
                        report_work::PAYLOAD_BYTES_HASHED.with(|n| n.set(n.get() + value.len()));
                        sha256_prefixed(value.as_bytes())
                    }),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let legal_animator_phase = matches!(
        stage,
        ProjectRuntimeSessionStage::AuiActionDispatch | ProjectRuntimeSessionStage::FixedUpdate
    );
    let snapshots = if legal_animator_phase {
        spine
            .as_mut()
            .map(|m| m.callback_snapshots(world))
            .transpose()
    } else {
        Ok(None)
    };
    let snapshot_error = snapshots.as_ref().err().cloned();
    session.accept_animator_snapshots(snapshots.ok().flatten().unwrap_or_default());
    let callback = catch_unwind(AssertUnwindSafe(|| {
        let context = ProjectRuntimeSessionContext {
            frame_index,
            time,
            world: WorldReadApi::with_runtime_cells(
                world,
                runtime_context.as_ref().map(|r| &*r.instance_loader),
            )
            .with_physics(physics),
        };
        match stage {
            ProjectRuntimeSessionStage::AuiActionDispatch => {
                session.handle_aui_actions(context, ProjectAuiActionBatch::new(actions))
            }
            ProjectRuntimeSessionStage::FixedUpdate => session.fixed_update(context),
            ProjectRuntimeSessionStage::FocusChanged { focused } => session.lifecycle(
                context,
                ProjectRuntimeLifecycleEvent::FocusChanged { focused },
            ),
        }
    }));
    let Ok(mut output) = callback else {
        session.finish_ui_updates(false);
        return ProjectRuntimeSessionStageReport {
            stage,
            status: ProjectRuntimeSessionStatus::Faulted,
            action_count: actions.len(),
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            staged_mutation_count: 0,
            committed_mutation_count: 0,
            rejected_mutation_count: 0,
            diagnostics: vec!["project_runtime.session_panicked".to_string()],
            action_trace,
            terminal_fault: true,
        };
    };

    let requested_control = output.host_control;
    let status = output.status;
    let handled_action_count = output.handled_action_count;
    let unhandled_action_count = output.unhandled_action_count;
    let rejected_action_count = output.rejected_action_count;
    let staged_mutation_count = output.mutations.len();
    let mut diagnostics = output
        .diagnostics
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let preparation = if status == ProjectRuntimeSessionStatus::Applied {
        let animator_check = if let Some(error) = snapshot_error {
            Err(error)
        } else if output.mutations.animator_commands.is_empty() {
            Ok(())
        } else if !legal_animator_phase {
            Err("animator.runtime.invalid_phase".into())
        } else if let Some(module) = spine.as_ref() {
            let tick = if stage == ProjectRuntimeSessionStage::FixedUpdate {
                time.fixed_frame_count
            } else {
                time.fixed_frame_count.saturating_add(1)
            };
            module.preflight(world, tick, &output.mutations.animator_commands)
        } else {
            Err("animator.runtime.registry_uninitialized".into())
        };
        match animator_check {
            Ok(()) => {
                output.mutations.animator_validated = true;
                output.mutations.commit_with_runtime(world, runtime_context)
            }
            Err(message) => {
                diagnostics.push(message.clone());
                Err(ProjectRuntimeMutationError {
                    code: "animator.runtime.preflight_failed",
                    operation_index: 0,
                    message,
                    report: ProjectRuntimeMutationCommitReport::rejected(staged_mutation_count),
                })
            }
        }
    } else {
        Ok((
            ProjectRuntimeMutationCommitReport::rejected(staged_mutation_count),
            None,
        ))
    };
    let (committed_mutation_count, rejected_mutation_count, terminal_fault) = match preparation {
        Ok((report, receipts)) => {
            diagnostics.extend(report.generated_content_errors.iter().map(ToString::to_string));
            if let Some(receipts) = receipts {
                session.accept_spawn_receipts(receipts);
            }
            if let Some(module) = spine.as_mut() {
                module.queue(report.animator_commands);
            }
            animator2d_commands.extend(report.animator2d_commands.iter().cloned());
            audio_source_commands.extend(report.audio_source_commands);
            particle_commands.extend(report.particle_commands);
            (report.committed_count, report.rejected_count, false)
        }
        Err(error) => {
            diagnostics.push(error.code.to_string());
            (
                error.report.committed_count,
                error.report.rejected_count,
                true,
            )
        }
    };
    let terminal_fault = terminal_fault || status == ProjectRuntimeSessionStatus::Faulted;
    session.finish_ui_updates(
        status == ProjectRuntimeSessionStatus::Applied
            && !terminal_fault
            && rejected_mutation_count == 0
            && rejected_action_count == 0,
    );
    if status == ProjectRuntimeSessionStatus::Applied
        && !terminal_fault
        && rejected_mutation_count == 0
        && rejected_action_count == 0
    {
        *host_control = host_control.or(requested_control);
    }
    if status == ProjectRuntimeSessionStatus::Faulted
        && !diagnostics
            .iter()
            .any(|code| code == "project_runtime.session_faulted")
    {
        diagnostics.push("project_runtime.session_faulted".to_string());
    }

    ProjectRuntimeSessionStageReport {
        stage,
        status,
        action_count: actions.len(),
        handled_action_count,
        unhandled_action_count,
        rejected_action_count,
        staged_mutation_count,
        committed_mutation_count,
        rejected_mutation_count,
        diagnostics,
        action_trace,
        terminal_fault,
    }
}

#[derive(Debug)]
pub struct ProjectRuntimeSessionOutput {
    pub host_control: Option<ProjectRuntimeHostControl>,
    pub status: ProjectRuntimeSessionStatus,
    pub handled_action_count: usize,
    pub unhandled_action_count: usize,
    pub rejected_action_count: usize,
    pub mutations: ProjectRuntimeMutationBuffer,
    pub diagnostics: Vec<&'static str>,
}

impl ProjectRuntimeSessionOutput {
    pub fn no_op() -> Self {
        Self {
            status: ProjectRuntimeSessionStatus::NoOp,
            host_control: None,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations: ProjectRuntimeMutationBuffer::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn applied(mutations: ProjectRuntimeMutationBuffer) -> Self {
        Self {
            status: ProjectRuntimeSessionStatus::Applied,
            host_control: None,
            handled_action_count: 0,
            unhandled_action_count: 0,
            rejected_action_count: 0,
            mutations,
            diagnostics: Vec::new(),
        }
    }

    pub fn prepare_mutations(
        self,
        world: &World,
    ) -> Result<ProjectRuntimeMutationPreparation, ProjectRuntimeMutationError> {
        if self.status == ProjectRuntimeSessionStatus::Applied {
            return self
                .mutations
                .prepare(world)
                .map(ProjectRuntimeMutationPreparation::Prepared);
        }
        Ok(ProjectRuntimeMutationPreparation::Dropped(
            ProjectRuntimeMutationCommitReport::rejected(self.mutations.len()),
        ))
    }
}

pub trait ProjectRuntimeSession: Send {
    fn accept_animator_snapshots(
        &mut self,
        _snapshots: crate::spine_animation::session::CallbackSnapshots,
    ) {
    }

    /// Opt-in borrowed census of host-owned backing. Never called by the Off
    /// frame path; module-private allocations require their own observation.
    fn observe_memory(&self, _sample: &mut crate::resource_memory::ResourceMemorySample) {}

    fn mount_ui_documents(
        &mut self,
        _documents: &crate::runtime_package::RuntimeAuiDocumentRegistry,
    ) -> Result<(), ProjectRuntimeSessionFactoryError> {
        Ok(())
    }

    fn retained_ui(&self) -> Option<&crate::aui_session::AuiSession> {
        None
    }
    fn publish_ui_geometry(&mut self, _output: &crate::aui::AuiRuntimePresentOutput) {}
    fn advance_ui_animations(
        &mut self,
        _frame: u64,
        _delta_us: u64,
    ) -> Result<(), project_runtime_sdk::aui::UiError> {
        Ok(())
    }
    fn cancel_drag_animations(
        &mut self,
        _document: &str,
        _interaction: &crate::aui::AuiInteractionResult,
    ) {
    }

    fn aui_extension_stats(&self) -> ProjectRuntimeAuiExtensionStats {
        ProjectRuntimeAuiExtensionStats::default()
    }

    fn refresh_ui_extensions_for_frame(
        &mut self,
        _frame: &crate::aui_extension::AuiCustomFrame,
        _allow_publish: bool,
    ) -> Result<crate::aui_extension::AuiCustomFrameResult, String> {
        Ok(crate::aui_extension::AuiCustomFrameResult::default())
    }

    /// Called once after the host accepts or rejects the containing session callback.
    fn finish_ui_updates(&mut self, _accepted: bool) {}

    fn accept_spawn_receipts(&mut self, _receipts: BTreeMap<u64, (EntityId, u64)>) {}

    fn lifecycle(
        &mut self,
        _context: ProjectRuntimeSessionContext<'_>,
        _event: ProjectRuntimeLifecycleEvent,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }

    fn session_id(&self) -> &str;

    fn handle_aui_actions(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput;

    fn fixed_update(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput;

    fn observe(
        &self,
        _context: ProjectRuntimeObservationContext<'_>,
    ) -> ProjectRuntimeObservationOutput {
        ProjectRuntimeObservationOutput::empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeSessionFactoryError {
    pub message: String,
}

impl ProjectRuntimeSessionFactoryError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ProjectRuntimeSessionFactoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ProjectRuntimeSessionFactoryError {}

pub type ProjectRuntimeSessionFactory = std::sync::Arc<
    dyn for<'a> Fn(
            ProjectRuntimeSessionCreateContext<'a>,
        )
            -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError>
        + Send
        + Sync,
>;

#[derive(Default)]
pub struct EmptyProjectRuntimeSession;

impl ProjectRuntimeSession for EmptyProjectRuntimeSession {
    fn session_id(&self) -> &str {
        EMPTY_PROJECT_RUNTIME_SESSION_ID
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
        ProjectRuntimeSessionOutput::no_op()
    }
}

pub fn create_empty_project_runtime_session(
    _context: ProjectRuntimeSessionCreateContext<'_>,
) -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError> {
    Ok(Box::new(EmptyProjectRuntimeSession))
}

#[cfg(test)]
#[cfg_attr(test, path = "project_runtime_session/tests.rs")]
mod tests;

#[cfg(test)]
mod audio_source_tests {
    use super::*;
    use crate::audio::AudioSource;
    use crate::components::Hierarchy;
    use crate::runtime_package::RuntimeAssetRef;

    fn source_world(enabled: bool, with_source: bool) -> World {
        let mut world = World::new();
        let id = EntityId::from("speaker");
        world.spawn_entity(
            id.clone(),
            "Speaker",
            "audio",
            enabled,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        if with_source {
            world.insert_component_value(
                id,
                ComponentValue::AudioSource(AudioSource {
                    clip_ref: RuntimeAssetRef {
                        id: "audio-test".into(),
                        asset_type: "audio".into(),
                        guid: None,
                        sub_asset: None,
                    },
                    volume: 0.5,
                }),
            );
        }
        world
    }

    fn play_buffer() -> ProjectRuntimeMutationBuffer {
        let mut mutations = ProjectRuntimeMutationBuffer::new();
        mutations.audio_source_command(EntityId::from("speaker"), AudioSourceAction::Play);
        mutations
    }

    #[test]
    fn audio_source_noop_rejected_and_faulted_outputs_never_commit_commands() {
        let world = source_world(true, true);
        for status in [
            ProjectRuntimeSessionStatus::NoOp,
            ProjectRuntimeSessionStatus::Unhandled,
            ProjectRuntimeSessionStatus::Rejected,
            ProjectRuntimeSessionStatus::Faulted,
        ] {
            let mut output = ProjectRuntimeSessionOutput::applied(play_buffer());
            output.status = status;
            let ProjectRuntimeMutationPreparation::Dropped(report) =
                output.prepare_mutations(&world).unwrap()
            else {
                panic!("non-Applied output must drop audio");
            };
            assert_eq!(report.committed_count, 0);
            assert_eq!(report.rejected_count, 1);
            assert!(report.audio_source_commands.is_empty());
        }
    }

    #[test]
    fn audio_source_preparation_rejects_missing_disabled_and_untyped_targets() {
        for world in [
            World::new(),
            source_world(false, true),
            source_world(true, false),
        ] {
            let error = play_buffer().prepare(&world).unwrap_err();
            assert_eq!(error.code, "audio_source.target_unavailable");
            assert!(error.message.contains("speaker"));
            assert_eq!(error.report.committed_count, 0);
            assert!(error.report.audio_source_commands.is_empty());
        }
    }

    #[test]
    fn audio_source_failed_mutation_batch_does_not_leak_a_play_command() {
        let world = source_world(true, true);
        let mut buffer = play_buffer();
        buffer.write_transform(
            EntityId::from("missing"),
            Transform {
                local_position: crate::math::Vec3::ZERO,
                local_rotation: crate::math::Vec3::ZERO,
                local_scale: crate::math::Vec3::ONE,
            },
        );
        let error = buffer.prepare(&world).unwrap_err();
        assert_eq!(error.report.staged_count, 2);
        assert_eq!(error.report.committed_count, 0);
        assert!(error.report.audio_source_commands.is_empty());
    }

    #[test]
    fn audio_source_prepared_command_keeps_original_runtime_generation() {
        let mut world = source_world(true, true);
        let id = EntityId::from("speaker");
        let original = world.runtime_id_for_source(&id).unwrap();
        let prepared = play_buffer().prepare(&world).unwrap();
        world.despawn_entity(&id);
        let replacement = world.spawn_entity(
            id.clone(),
            "Replacement",
            "audio",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        );
        let report = prepared.commit(&mut world).unwrap();
        assert_ne!(original, replacement);
        assert_eq!(report.audio_source_commands[0].runtime_id, original);
        assert_ne!(report.audio_source_commands[0].runtime_id, replacement);
    }
}
