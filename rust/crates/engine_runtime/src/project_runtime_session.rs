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
use crate::runtime_time::TimeContext;
use crate::world::World;
use crate::world_api::{
    prepare_component_field_write, WorldApiError, WorldReadApi, WorldWriteApi, WorldWriteRecord,
};
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
    operations: Vec<ProjectRuntimeMutationOperation>,
    animator2d_commands: Vec<Animator2DCommand>,
}

impl ProjectRuntimeMutationBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty() && self.animator2d_commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.operations.len() + self.animator2d_commands.len()
    }

    pub fn replace_component(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    ) {
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
        self.operations
            .push(ProjectRuntimeMutationOperation::WriteComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            });
    }

    pub fn write_transform(&mut self, entity_id: EntityId, transform: Transform) {
        self.operations
            .push(ProjectRuntimeMutationOperation::WriteTransform {
                entity_id,
                transform,
            });
    }

    pub fn push_gameplay_command(&mut self, command: GameplayCommand) {
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
        let staged_count = self.len();
        let animator2d_commands = self.animator2d_commands;
        let mut overlay = BTreeMap::<(EntityId, ComponentTypeId), ComponentValue>::new();
        let mut prepared = Vec::with_capacity(staged_count);

        for (operation_index, operation) in self.operations.into_iter().enumerate() {
            let prepared_operation = match operation {
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
                ProjectRuntimeMutationOperation::GameplayCommand(_) => {
                    return Err(ProjectRuntimeMutationError::unsupported(
                        operation_index,
                        staged_count,
                    ));
                }
            };
            prepared.push(prepared_operation);
        }

        Ok(PreparedProjectRuntimeMutationBatch {
            operations: prepared,
            animator2d_commands,
            staged_count,
        })
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
    operations: Vec<PreparedProjectRuntimeMutation>,
    animator2d_commands: Vec<Animator2DCommand>,
    staged_count: usize,
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
        let mut records = Vec::with_capacity(self.operations.len());
        let mut write_api = WorldWriteApi::new(world);
        for (operation_index, operation) in self.operations.into_iter().enumerate() {
            let record = match operation {
                PreparedProjectRuntimeMutation::WriteComponent {
                    entity_id,
                    component_type,
                    value,
                    field,
                } => {
                    let mut record = write_api
                        .write_component(entity_id, component_type, value)
                        .map_err(|error| {
                            ProjectRuntimeMutationError::commit_failed(
                                operation_index,
                                self.staged_count,
                                records.len(),
                                error,
                            )
                        })?;
                    record.field = field;
                    record
                }
            };
            records.push(record);
        }
        Ok(ProjectRuntimeMutationCommitReport {
            staged_count: self.staged_count,
            committed_count: records.len() + self.animator2d_commands.len(),
            rejected_count: 0,
            records,
            animator2d_commands: self.animator2d_commands,
        })
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
                staged_count,
                committed_count,
                rejected_count: staged_count.saturating_sub(committed_count),
                records: Vec::new(),
                animator2d_commands: Vec::new(),
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
    pub staged_count: usize,
    pub committed_count: usize,
    pub rejected_count: usize,
    pub records: Vec<WorldWriteRecord>,
    pub animator2d_commands: Vec<Animator2DCommand>,
}

impl ProjectRuntimeMutationCommitReport {
    fn rejected(staged_count: usize) -> Self {
        Self {
            staged_count,
            committed_count: 0,
            rejected_count: staged_count,
            records: Vec::new(),
            animator2d_commands: Vec::new(),
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
}

impl ProjectRuntimeSessionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuiActionDispatch => "aui_action_dispatch",
            Self::FixedUpdate => "fixed_update",
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

pub(crate) fn execute_project_runtime_session_stage_with_animator2d(
    session: &mut dyn ProjectRuntimeSession,
    stage: ProjectRuntimeSessionStage,
    frame_index: u64,
    time: TimeContext,
    world: &mut World,
    actions: &[AuiAction],
    report_level: ProjectRuntimeSessionReportLevel,
    animator2d_commands: &mut Vec<Animator2DCommand>,
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
                    payload_digest: payload.map(|value| sha256_prefixed(value.as_bytes())),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let callback = catch_unwind(AssertUnwindSafe(|| {
        let context = ProjectRuntimeSessionContext {
            frame_index,
            time,
            world: WorldReadApi::new(world),
        };
        match stage {
            ProjectRuntimeSessionStage::AuiActionDispatch => {
                session.handle_aui_actions(context, ProjectAuiActionBatch::new(actions))
            }
            ProjectRuntimeSessionStage::FixedUpdate => session.fixed_update(context),
        }
    }));
    let Ok(output) = callback else {
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
    let preparation = output.prepare_mutations(world);
    let (committed_mutation_count, rejected_mutation_count, terminal_fault) = match preparation {
        Ok(ProjectRuntimeMutationPreparation::Prepared(batch)) => match batch.commit(world) {
            Ok(report) => {
                animator2d_commands.extend(report.animator2d_commands.iter().cloned());
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
        },
        Ok(ProjectRuntimeMutationPreparation::Dropped(report)) => {
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

pub type ProjectRuntimeSessionFactory =
    for<'a> fn(
        ProjectRuntimeSessionCreateContext<'a>,
    ) -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeSessionFactoryError>;

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
mod tests;
