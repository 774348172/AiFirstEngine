use crate::archetype::ComponentValue;
use crate::component_value::RuntimeValue;
use crate::components::ComponentTypeId;
use crate::field_path::FieldPath;
use crate::gameplay_command::{GameplayCommand, GameplayCommandBuffer, GameplayCommandId};
use crate::ids::{EntityId, SourceEntityId};
use crate::input_action::ActionSnapshot;
use crate::math::Vec3;
use crate::physics2d::CollisionPair;
use crate::query::QuerySpec;
use crate::runtime_instance::RuntimeInstanceId;
use crate::runtime_package::RuntimeAssetRef;
use crate::runtime_time::TimeContext;
use crate::world_api::{WorldApiError, WorldWriteApi, WorldWriteRecord};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePhase {
    FixedUpdate,
    FrameUpdate,
    PostPhysics,
    EventHandler,
}

impl RulePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedUpdate => "FixedUpdate",
            Self::FrameUpdate => "Update",
            Self::PostPhysics => "PostPhysics",
            Self::EventHandler => "EventHandler",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorKind {
    RustAot,
    IrInterpreter,
}

impl ExecutorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustAot => "RustAotExecutor",
            Self::IrInterpreter => "ValidationOnlyIrInterpreter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicStatus {
    Applied,
    Skipped,
    Failed,
    Unsupported,
}

impl LogicStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicError {
    pub code: &'static str,
    pub message: String,
}

impl From<WorldApiError> for LogicError {
    fn from(error: WorldApiError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicWrite {
    pub entity_id: EntityId,
    pub component_type: ComponentTypeId,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl From<WorldWriteRecord> for LogicWrite {
    fn from(record: WorldWriteRecord) -> Self {
        Self {
            entity_id: record.entity_id,
            component_type: record.component_type,
            field: record.field,
            before: record.before,
            after: record.after,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicRead {
    pub entity_id: EntityId,
    pub component_type: ComponentTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicQuery {
    pub all: Vec<ComponentTypeId>,
    pub none: Vec<ComponentTypeId>,
    pub result_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicResult {
    pub rule_id: String,
    pub executor_kind: ExecutorKind,
    pub status: LogicStatus,
    pub queries: Vec<LogicQuery>,
    pub reads: Vec<LogicRead>,
    pub writes: Vec<LogicWrite>,
    pub command_ids: Vec<GameplayCommandId>,
    pub errors: Vec<LogicError>,
}

impl LogicResult {
    pub fn applied(rule_id: impl Into<String>, executor_kind: ExecutorKind) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_kind,
            status: LogicStatus::Applied,
            queries: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            command_ids: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn skipped(rule_id: impl Into<String>, executor_kind: ExecutorKind) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_kind,
            status: LogicStatus::Skipped,
            queries: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            command_ids: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn failed(
        rule_id: impl Into<String>,
        executor_kind: ExecutorKind,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_kind,
            status: LogicStatus::Failed,
            queries: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            command_ids: Vec::new(),
            errors: vec![LogicError {
                code,
                message: message.into(),
            }],
        }
    }

    pub fn unsupported(rule_id: impl Into<String>, executor_kind: ExecutorKind) -> Self {
        Self {
            rule_id: rule_id.into(),
            executor_kind,
            status: LogicStatus::Unsupported,
            queries: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            command_ids: Vec::new(),
            errors: vec![LogicError {
                code: "unsupported_executor",
                message:
                    "IR interpreter is validation-only in v1 and is not a runtime execution path"
                        .to_string(),
            }],
        }
    }
}

pub struct LogicContext<'a> {
    pub frame_index: u64,
    pub delta_time: f32,
    pub phase: RulePhase,
    time_context: TimeContext,
    action_snapshot: Option<&'a ActionSnapshot>,
    collision_pairs: &'a [CollisionPair],
    world: WorldWriteApi<'a>,
    commands: GameplayCommandBuffer,
    queries: Vec<LogicQuery>,
    reads: Vec<LogicRead>,
}

impl<'a> LogicContext<'a> {
    pub fn new(
        frame_index: u64,
        delta_time: f32,
        phase: RulePhase,
        world: WorldWriteApi<'a>,
    ) -> Self {
        let time_context =
            TimeContext::from_delta(frame_index, delta_time, phase == RulePhase::FixedUpdate);
        Self::with_time_context(frame_index, time_context, phase, world)
    }

    pub fn with_time_context(
        frame_index: u64,
        time_context: TimeContext,
        phase: RulePhase,
        world: WorldWriteApi<'a>,
    ) -> Self {
        Self {
            frame_index,
            delta_time: time_context.delta_time,
            phase,
            time_context,
            action_snapshot: None,
            collision_pairs: &[],
            world,
            commands: GameplayCommandBuffer::new(),
            queries: Vec::new(),
            reads: Vec::new(),
        }
    }

    pub fn time(&self) -> &TimeContext {
        &self.time_context
    }

    pub fn with_action_snapshot(mut self, action_snapshot: Option<&'a ActionSnapshot>) -> Self {
        self.action_snapshot = action_snapshot;
        self
    }

    pub fn with_collision_pairs(mut self, collision_pairs: &'a [CollisionPair]) -> Self {
        self.collision_pairs = collision_pairs;
        self
    }

    pub fn action_snapshot(&self) -> Option<&ActionSnapshot> {
        self.action_snapshot
    }

    pub fn action_pressed(&self, action_id: &str) -> bool {
        self.action_snapshot
            .is_some_and(|snapshot| snapshot.button_pressed(action_id))
    }

    pub fn collision_pairs(&self) -> &[CollisionPair] {
        self.collision_pairs
    }

    pub fn read_transform_local_position(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec3, WorldApiError> {
        Ok(self.world.read_transform(entity_id)?.local_position)
    }

    pub fn query(&mut self, spec: QuerySpec) -> Vec<EntityId> {
        let result = self.world.query(&spec);
        self.queries.push(LogicQuery {
            all: spec.all,
            none: spec.none,
            result_count: result.len(),
        });
        result
    }

    pub fn read_component(
        &mut self,
        entity_id: &EntityId,
        component_type: &ComponentTypeId,
    ) -> Result<ComponentValue, WorldApiError> {
        let value = self.world.read_component(entity_id, component_type)?;
        self.reads.push(LogicRead {
            entity_id: entity_id.clone(),
            component_type: component_type.clone(),
        });
        Ok(value)
    }

    pub fn write_component(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        value: ComponentValue,
    ) -> Result<LogicWrite, WorldApiError> {
        self.world
            .write_component(entity_id, component_type, value)
            .map(LogicWrite::from)
    }

    pub fn write_component_field(
        &mut self,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: &FieldPath,
        value: RuntimeValue,
    ) -> Result<LogicWrite, WorldApiError> {
        self.world
            .write_component_field(entity_id, component_type, field_path, value)
            .map(LogicWrite::from)
    }

    pub fn write_transform_local_position(
        &mut self,
        entity_id: EntityId,
        local_position: Vec3,
    ) -> Result<LogicWrite, WorldApiError> {
        self.world
            .write_transform_local_position(entity_id, local_position)
            .map(LogicWrite::from)
    }

    pub fn commands(&mut self) -> &mut GameplayCommandBuffer {
        &mut self.commands
    }

    pub fn enqueue_command(&mut self, command: GameplayCommand) -> GameplayCommandId {
        self.commands.push(command)
    }

    pub fn request_instantiate_prefab(
        &mut self,
        prefab_ref: RuntimeAssetRef,
        parent_entity: Option<SourceEntityId>,
        target_scene_instance: Option<RuntimeInstanceId>,
    ) -> GameplayCommandId {
        self.enqueue_command(GameplayCommand::InstantiatePrefab {
            prefab_ref,
            parent_entity,
            target_scene_instance,
        })
    }

    pub fn request_despawn_prefab_instance(
        &mut self,
        instance_id: RuntimeInstanceId,
    ) -> GameplayCommandId {
        self.enqueue_command(GameplayCommand::DespawnPrefabInstance { instance_id })
    }

    pub fn request_despawn_entity(&mut self, entity_id: EntityId) -> GameplayCommandId {
        self.enqueue_command(GameplayCommand::DespawnEntity { entity_id })
    }

    pub fn take_commands(&mut self) -> Vec<(GameplayCommandId, GameplayCommand)> {
        self.commands.drain()
    }

    pub fn take_queries(&mut self) -> Vec<LogicQuery> {
        std::mem::take(&mut self.queries)
    }

    pub fn take_reads(&mut self) -> Vec<LogicRead> {
        std::mem::take(&mut self.reads)
    }
}

pub trait LogicExecutor {
    fn executor_kind(&self) -> ExecutorKind;
    fn run(&self, rule_id: &str, context: &mut LogicContext<'_>) -> LogicResult;
}

#[derive(Clone)]
pub struct RustAotRule {
    callback: Arc<dyn for<'a> Fn(&mut LogicContext<'a>) -> LogicResult + Send + Sync>,
}

impl RustAotRule {
    pub fn new(
        callback: impl for<'a> Fn(&mut LogicContext<'a>) -> LogicResult + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    fn run(&self, context: &mut LogicContext<'_>) -> LogicResult {
        (self.callback)(context)
    }
}

impl fmt::Debug for RustAotRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RustAotRule(..)")
    }
}

#[derive(Clone, Default, Debug)]
pub struct RustAotExecutor {
    rules: BTreeMap<String, RustAotRule>,
}

impl RustAotExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_rule(&mut self, rule_id: impl Into<String>, rule: RustAotRule) {
        self.rules.insert(rule_id.into(), rule);
    }
}

impl LogicExecutor for RustAotExecutor {
    fn executor_kind(&self) -> ExecutorKind {
        ExecutorKind::RustAot
    }

    fn run(&self, rule_id: &str, context: &mut LogicContext<'_>) -> LogicResult {
        let Some(rule) = self.rules.get(rule_id) else {
            return LogicResult::failed(
                rule_id,
                self.executor_kind(),
                "missing_rule",
                "Rust AOT rule is not registered",
            );
        };
        rule.run(context)
    }
}

/// Validation-only placeholder for future diagnostics/hotfix experiments.
///
/// RuntimePackage execution is intentionally restricted to `RustAot` in v1.
/// This type exists only so tests and reports can emit structured unsupported
/// evidence instead of silently accepting an interpreter path.
#[derive(Clone, Default, Debug)]
pub(crate) struct IrInterpreterExecutor;

impl IrInterpreterExecutor {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl LogicExecutor for IrInterpreterExecutor {
    fn executor_kind(&self) -> ExecutorKind {
        ExecutorKind::IrInterpreter
    }

    fn run(&self, rule_id: &str, _context: &mut LogicContext<'_>) -> LogicResult {
        LogicResult::unsupported(rule_id, self.executor_kind())
    }
}
