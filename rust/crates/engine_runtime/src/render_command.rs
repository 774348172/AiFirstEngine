use crate::components::Transform;
use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::projection::{ProjectionDomain, ProjectionKind, ProjectionReport};
use crate::render_state::{
    RenderPayloadKind, RenderProxy, RenderProxyDescriptor, RenderProxyId, RenderSceneState,
};
use crate::world::DirtyType;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderCommandId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderCommandType {
    RemoveProxy,
    AddProxy,
    UpdateRenderState,
    UpdateTransform,
    UpdateDynamicData,
    UpdateInstanceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderCommandPayloadKind {
    Proxy,
    RenderState,
    Transform,
    DynamicData,
    InstanceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderCommandStage {
    Collect,
    Sort,
    Normalize,
    Merge,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandResult {
    Applied,
    Merged,
    Covered,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReasonCode {
    MissingProxy,
    MissingResource,
    PayloadKindConflict,
    UpdateAfterRemove,
    AddExistingProxy,
    RemoveMissingProxy,
    CoveredByRemove,
    CoveredByNoop,
    MergedLastValueWins,
    InvalidPayload,
    ApplyFailed,
    FallbackUsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCommandDiagnostic {
    pub diagnostic_id: u64,
    pub frame_index: u64,
    pub severity: DiagnosticSeverity,
    pub code: &'static str,
    pub stage: RenderCommandStage,
    pub runtime_entity_id: Option<RuntimeEntityId>,
    pub source_entity_id: Option<SourceEntityId>,
    pub proxy_id: Option<RenderProxyId>,
    pub command_id: Option<RenderCommandId>,
    pub command_type: Option<RenderCommandType>,
    pub payload_kind: Option<RenderCommandPayloadKind>,
    pub result: CommandResult,
    pub reason_code: ReasonCode,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderFrameReportLevel {
    Off,
    Stats,
    Summary,
    Evidence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderFrameCounters {
    pub raw_command_count: usize,
    pub merged_command_count: usize,
    pub applied_command_count: usize,
    pub covered_command_count: usize,
    pub skipped_command_count: usize,
    pub failed_command_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub missing_proxy_count: usize,
    pub missing_resource_count: usize,
    pub fallback_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedRenderEntity {
    pub runtime_entity_id: RuntimeEntityId,
    pub source_entity_id: SourceEntityId,
    pub proxy_id: Option<RenderProxyId>,
    pub change_kind: RenderCommandType,
    pub result: CommandResult,
    pub reason_code: Option<ReasonCode>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvent {
    pub severity: DiagnosticSeverity,
    pub event_code: &'static str,
    pub stage: RenderCommandStage,
    pub runtime_entity_id: Option<RuntimeEntityId>,
    pub proxy_id: Option<RenderProxyId>,
    pub resource_id: Option<String>,
    pub command_type: Option<RenderCommandType>,
    pub reason_code: ReasonCode,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRef {
    pub trace_id: String,
    pub source_system: Option<String>,
    pub source_patch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrameReport {
    pub frame_index: u64,
    pub report_level: RenderFrameReportLevel,
    pub counters: RenderFrameCounters,
    pub changed_entities: Vec<ChangedRenderEntity>,
    pub render_events: Vec<RenderEvent>,
    pub trace_refs: Vec<TraceRef>,
}

impl RenderFrameReport {
    pub fn from_commands_and_diagnostics(
        frame_index: u64,
        report_level: RenderFrameReportLevel,
        raw_command_count: usize,
        merged_commands: &[RenderCommand],
        diagnostics: &[RenderCommandDiagnostic],
    ) -> Self {
        let mut counters = RenderFrameCounters {
            raw_command_count,
            merged_command_count: merged_commands.len(),
            applied_command_count: merged_commands.len(),
            ..RenderFrameCounters::default()
        };
        let mut changed_entities = Vec::new();
        let mut render_events = Vec::new();
        let mut trace_refs = BTreeMap::<String, TraceRef>::new();

        for command in merged_commands {
            if let Some(trace_id) = &command.trace_id {
                trace_refs.entry(trace_id.clone()).or_insert(TraceRef {
                    trace_id: trace_id.clone(),
                    source_system: None,
                    source_patch: None,
                });
            }
            if report_level >= RenderFrameReportLevel::Summary {
                changed_entities.push(ChangedRenderEntity {
                    runtime_entity_id: command.runtime_entity_id,
                    source_entity_id: command.source_entity_id.clone(),
                    proxy_id: command.proxy_id,
                    change_kind: command.command_type,
                    result: CommandResult::Applied,
                    reason_code: None,
                    trace_id: command.trace_id.clone(),
                });
            }
        }

        for diagnostic in diagnostics {
            match diagnostic.result {
                CommandResult::Merged => counters.covered_command_count += 1,
                CommandResult::Covered => counters.covered_command_count += 1,
                CommandResult::Skipped => counters.skipped_command_count += 1,
                CommandResult::Failed => counters.failed_command_count += 1,
                CommandResult::Applied => {}
            }
            match diagnostic.severity {
                DiagnosticSeverity::Info => {}
                DiagnosticSeverity::Warning => counters.warning_count += 1,
                DiagnosticSeverity::Error => counters.error_count += 1,
            }
            match diagnostic.reason_code {
                ReasonCode::MissingProxy | ReasonCode::RemoveMissingProxy => {
                    counters.missing_proxy_count += 1;
                }
                ReasonCode::MissingResource => counters.missing_resource_count += 1,
                ReasonCode::FallbackUsed => counters.fallback_count += 1,
                _ => {}
            }
            if report_level >= RenderFrameReportLevel::Summary {
                render_events.push(RenderEvent {
                    severity: diagnostic.severity,
                    event_code: diagnostic.code,
                    stage: diagnostic.stage,
                    runtime_entity_id: diagnostic.runtime_entity_id,
                    proxy_id: diagnostic.proxy_id,
                    resource_id: None,
                    command_type: diagnostic.command_type,
                    reason_code: diagnostic.reason_code,
                    trace_id: diagnostic.trace_id.clone(),
                });
            }
        }

        Self {
            frame_index,
            report_level,
            counters,
            changed_entities,
            render_events,
            trace_refs: trace_refs.into_values().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderCommandPayload {
    AddProxy {
        transform: Transform,
        descriptor: RenderProxyDescriptor,
    },
    RemoveProxy,
    UpdateRenderState {
        visible: bool,
        layer: String,
        payload_kind: RenderPayloadKind,
        descriptor: Option<RenderProxyDescriptor>,
    },
    UpdateTransform {
        transform: Transform,
    },
    UpdateDynamicData,
    UpdateInstanceData,
}

impl RenderCommandPayload {
    pub fn payload_kind(&self) -> RenderCommandPayloadKind {
        match self {
            Self::AddProxy { .. } | Self::RemoveProxy => RenderCommandPayloadKind::Proxy,
            Self::UpdateRenderState { .. } => RenderCommandPayloadKind::RenderState,
            Self::UpdateTransform { .. } => RenderCommandPayloadKind::Transform,
            Self::UpdateDynamicData => RenderCommandPayloadKind::DynamicData,
            Self::UpdateInstanceData => RenderCommandPayloadKind::InstanceData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderCommandSortKey {
    pub frame_index: u64,
    pub runtime_entity_id: RuntimeEntityId,
    pub command_id: RenderCommandId,
    pub lifecycle_order: u8,
    pub command_type_order: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderCommand {
    pub command_id: RenderCommandId,
    pub frame_index: u64,
    pub source_entity_id: SourceEntityId,
    pub runtime_entity_id: RuntimeEntityId,
    pub proxy_id: Option<RenderProxyId>,
    pub command_type: RenderCommandType,
    pub payload_kind: RenderCommandPayloadKind,
    pub payload: RenderCommandPayload,
    pub sort_key: RenderCommandSortKey,
    pub source_dirty_type: DirtyType,
    pub trace_id: Option<String>,
}

impl RenderCommand {
    pub fn new(
        command_id: RenderCommandId,
        frame_index: u64,
        source_entity_id: SourceEntityId,
        runtime_entity_id: RuntimeEntityId,
        proxy_id: Option<RenderProxyId>,
        command_type: RenderCommandType,
        payload: RenderCommandPayload,
        source_dirty_type: DirtyType,
    ) -> Self {
        let payload_kind = payload.payload_kind();
        Self {
            command_id,
            frame_index,
            source_entity_id,
            runtime_entity_id,
            proxy_id,
            command_type,
            payload_kind,
            payload,
            sort_key: RenderCommandSortKey {
                frame_index,
                lifecycle_order: lifecycle_order(command_type),
                runtime_entity_id,
                command_type_order: command_type_order(command_type),
                command_id,
            },
            source_dirty_type,
            trace_id: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThreadLocalCommandBuffer {
    commands: Vec<RenderCommand>,
}

impl ThreadLocalCommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }

    pub fn into_commands(self) -> Vec<RenderCommand> {
        self.commands
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderCommandQueue {
    pub frame_index: u64,
    pub pending_commands: Vec<RenderCommand>,
    pub diagnostics: Vec<RenderCommandDiagnostic>,
}

impl RenderCommandQueue {
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            pending_commands: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn collect(&mut self, buffers: Vec<ThreadLocalCommandBuffer>) {
        for buffer in buffers {
            self.pending_commands.extend(buffer.into_commands());
        }
    }

    pub fn stable_sort(&mut self) {
        self.pending_commands
            .sort_by_key(|command| command.sort_key);
    }

    pub fn normalize_merge(&mut self, scene: &RenderSceneState) -> Vec<RenderCommand> {
        self.stable_sort();
        let mut slots = BTreeMap::<RuntimeEntityId, ObjectCommandSlot>::new();
        for command in self.pending_commands.iter().cloned() {
            let existed_at_frame_start = scene.proxy_for_runtime(command.runtime_entity_id);
            slots
                .entry(command.runtime_entity_id)
                .or_insert_with(|| {
                    ObjectCommandSlot::new(command.runtime_entity_id, existed_at_frame_start)
                })
                .push(command, &mut self.diagnostics);
        }
        let mut merged = Vec::new();
        for slot in slots.into_values() {
            merged.extend(slot.into_commands(&mut self.diagnostics));
        }
        merged.sort_by_key(|command| command.sort_key);
        merged
    }

    pub fn build_report(
        &self,
        report_level: RenderFrameReportLevel,
        merged_commands: &[RenderCommand],
        apply_diagnostics: &[RenderCommandDiagnostic],
    ) -> RenderFrameReport {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend_from_slice(apply_diagnostics);
        RenderFrameReport::from_commands_and_diagnostics(
            self.frame_index,
            report_level,
            self.pending_commands.len(),
            merged_commands,
            &diagnostics,
        )
    }

    pub fn projection_summary(&self) -> ProjectionReport {
        let error_count = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .count();
        let skipped_count = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.result == CommandResult::Skipped)
            .count();
        ProjectionReport::new(
            ProjectionKind::Render,
            ProjectionDomain::World,
            ProjectionDomain::Render,
            "RenderProjectionAdapter<DirtyRecord>",
        )
        .with_counts(self.pending_commands.len(), skipped_count, error_count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleState {
    None,
    Add,
    Remove,
    Recreate,
    NoOp,
}

#[derive(Debug, Clone)]
struct ObjectCommandSlot {
    runtime_entity_id: RuntimeEntityId,
    proxy_id: Option<RenderProxyId>,
    existed_at_frame_start: bool,
    lifecycle: LifecycleState,
    add_proxy: Option<RenderCommand>,
    remove_proxy: Option<RenderCommand>,
    render_state_payload: Option<RenderCommand>,
    transform_payload: Option<RenderCommand>,
    dynamic_data_payload: Option<RenderCommand>,
    instance_data_payload: Option<RenderCommand>,
}

impl ObjectCommandSlot {
    fn new(runtime_entity_id: RuntimeEntityId, proxy_id: Option<RenderProxyId>) -> Self {
        Self {
            runtime_entity_id,
            proxy_id,
            existed_at_frame_start: proxy_id.is_some(),
            lifecycle: LifecycleState::None,
            add_proxy: None,
            remove_proxy: None,
            render_state_payload: None,
            transform_payload: None,
            dynamic_data_payload: None,
            instance_data_payload: None,
        }
    }

    fn push(&mut self, command: RenderCommand, diagnostics: &mut Vec<RenderCommandDiagnostic>) {
        match command.command_type {
            RenderCommandType::AddProxy => self.push_add(command, diagnostics),
            RenderCommandType::RemoveProxy => self.push_remove(command, diagnostics),
            RenderCommandType::UpdateRenderState => {
                self.cover_or_replace_update(command, UpdateKind::RenderState, diagnostics)
            }
            RenderCommandType::UpdateTransform => {
                self.cover_or_replace_update(command, UpdateKind::Transform, diagnostics)
            }
            RenderCommandType::UpdateDynamicData => {
                self.cover_or_replace_update(command, UpdateKind::DynamicData, diagnostics)
            }
            RenderCommandType::UpdateInstanceData => {
                self.cover_or_replace_update(command, UpdateKind::InstanceData, diagnostics)
            }
        }
    }

    fn push_add(&mut self, command: RenderCommand, diagnostics: &mut Vec<RenderCommandDiagnostic>) {
        if self.existed_at_frame_start && self.lifecycle != LifecycleState::Remove {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Warning,
                RenderCommandStage::Normalize,
                CommandResult::Skipped,
                ReasonCode::AddExistingProxy,
                "add_existing_proxy",
            ));
            self.render_state_payload = Some(as_render_state_update(command));
            return;
        }
        if self.lifecycle == LifecycleState::Remove {
            self.lifecycle = LifecycleState::Recreate;
            self.add_proxy = Some(command);
            return;
        }
        self.lifecycle = LifecycleState::Add;
        self.add_proxy = Some(command);
    }

    fn push_remove(
        &mut self,
        command: RenderCommand,
        diagnostics: &mut Vec<RenderCommandDiagnostic>,
    ) {
        if !self.existed_at_frame_start && self.lifecycle == LifecycleState::None {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Warning,
                RenderCommandStage::Normalize,
                CommandResult::Skipped,
                ReasonCode::RemoveMissingProxy,
                "remove_missing_proxy",
            ));
            self.lifecycle = LifecycleState::NoOp;
            return;
        }
        if self.lifecycle == LifecycleState::Add {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Info,
                RenderCommandStage::Merge,
                CommandResult::Covered,
                ReasonCode::CoveredByNoop,
                "add_then_remove_noop",
            ));
            self.lifecycle = LifecycleState::NoOp;
            self.add_proxy = None;
            self.remove_proxy = None;
            self.clear_updates();
            return;
        }
        self.lifecycle = LifecycleState::Remove;
        self.remove_proxy = Some(command);
        self.clear_updates();
    }

    fn cover_or_replace_update(
        &mut self,
        command: RenderCommand,
        update_kind: UpdateKind,
        diagnostics: &mut Vec<RenderCommandDiagnostic>,
    ) {
        if self.lifecycle == LifecycleState::Remove || self.lifecycle == LifecycleState::NoOp {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Info,
                RenderCommandStage::Merge,
                CommandResult::Covered,
                ReasonCode::CoveredByRemove,
                "update_covered",
            ));
            return;
        }
        if !self.existed_at_frame_start && self.lifecycle != LifecycleState::Add {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Warning,
                RenderCommandStage::Normalize,
                CommandResult::Skipped,
                ReasonCode::MissingProxy,
                "missing_proxy",
            ));
            return;
        }
        let target = match update_kind {
            UpdateKind::RenderState => &mut self.render_state_payload,
            UpdateKind::Transform => &mut self.transform_payload,
            UpdateKind::DynamicData => &mut self.dynamic_data_payload,
            UpdateKind::InstanceData => &mut self.instance_data_payload,
        };
        if target.is_some() {
            diagnostics.push(diagnostic(
                diagnostics.len() as u64 + 1,
                &command,
                DiagnosticSeverity::Info,
                RenderCommandStage::Merge,
                CommandResult::Merged,
                ReasonCode::MergedLastValueWins,
                "merged_last_value_wins",
            ));
        }
        *target = Some(command);
    }

    fn into_commands(self, diagnostics: &mut Vec<RenderCommandDiagnostic>) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        match self.lifecycle {
            LifecycleState::NoOp => {}
            LifecycleState::Add => {
                if let Some(mut add) = self.add_proxy {
                    merge_updates_into_add(
                        &mut add,
                        self.render_state_payload,
                        self.transform_payload,
                    );
                    commands.push(add);
                }
            }
            LifecycleState::Remove => {
                if let Some(remove) = self.remove_proxy {
                    commands.push(remove);
                }
            }
            LifecycleState::Recreate => {
                if let Some(remove) = self.remove_proxy {
                    commands.push(remove);
                }
                if let Some(mut add) = self.add_proxy {
                    merge_updates_into_add(
                        &mut add,
                        self.render_state_payload,
                        self.transform_payload,
                    );
                    commands.push(add);
                }
            }
            LifecycleState::None => {
                if self.existed_at_frame_start {
                    extend_if_some(&mut commands, self.render_state_payload);
                    extend_if_some(&mut commands, self.transform_payload);
                    extend_if_some(&mut commands, self.dynamic_data_payload);
                    extend_if_some(&mut commands, self.instance_data_payload);
                } else {
                    let _ = diagnostics;
                    debug_assert!(self.proxy_id.is_none());
                    debug_assert!(
                        commands.is_empty(),
                        "missing proxy updates should be skipped"
                    );
                }
            }
        }
        let _ = self.runtime_entity_id;
        commands
    }

    fn clear_updates(&mut self) {
        self.render_state_payload = None;
        self.transform_payload = None;
        self.dynamic_data_payload = None;
        self.instance_data_payload = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateKind {
    RenderState,
    Transform,
    DynamicData,
    InstanceData,
}

fn extend_if_some(commands: &mut Vec<RenderCommand>, command: Option<RenderCommand>) {
    if let Some(command) = command {
        commands.push(command);
    }
}

fn merge_updates_into_add(
    add: &mut RenderCommand,
    render_state: Option<RenderCommand>,
    transform: Option<RenderCommand>,
) {
    let RenderCommandPayload::AddProxy {
        transform: add_transform,
        descriptor,
    } = &mut add.payload
    else {
        return;
    };
    if let Some(transform_command) = transform {
        if let RenderCommandPayload::UpdateTransform { transform } = transform_command.payload {
            *add_transform = transform;
        }
    }
    if let Some(render_state_command) = render_state {
        if let RenderCommandPayload::UpdateRenderState {
            visible,
            layer,
            descriptor: update_descriptor,
            ..
        } = render_state_command.payload
        {
            descriptor.visible = visible;
            descriptor.layer = layer;
            if let Some(update_descriptor) = update_descriptor {
                descriptor.payload = update_descriptor.payload;
            }
        }
    }
}

fn as_render_state_update(mut command: RenderCommand) -> RenderCommand {
    if let RenderCommandPayload::AddProxy { descriptor, .. } = command.payload {
        command.command_type = RenderCommandType::UpdateRenderState;
        command.payload_kind = RenderCommandPayloadKind::RenderState;
        command.payload = RenderCommandPayload::UpdateRenderState {
            visible: descriptor.visible,
            layer: descriptor.layer.clone(),
            payload_kind: descriptor.payload_kind(),
            descriptor: Some(descriptor),
        };
        command.sort_key.command_type_order = command_type_order(command.command_type);
    }
    command
}

pub fn apply_batch(
    scene: &mut RenderSceneState,
    commands: &[RenderCommand],
) -> Vec<RenderCommandDiagnostic> {
    let mut diagnostics = Vec::new();
    for command in commands {
        match &command.payload {
            RenderCommandPayload::AddProxy {
                transform,
                descriptor,
            } => {
                if scene.proxy_for_runtime(command.runtime_entity_id).is_some() {
                    diagnostics.push(diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                        DiagnosticSeverity::Warning,
                        RenderCommandStage::Apply,
                        CommandResult::Skipped,
                        ReasonCode::AddExistingProxy,
                        "add_existing_proxy",
                    ));
                    continue;
                }
                let proxy = RenderProxy::from_descriptor(
                    RenderProxyId(0),
                    command.runtime_entity_id,
                    command.source_entity_id.clone(),
                    transform.clone(),
                    descriptor.clone(),
                );
                scene.insert_proxy(proxy);
            }
            RenderCommandPayload::RemoveProxy => {
                let proxy_id = command
                    .proxy_id
                    .or_else(|| scene.proxy_for_runtime(command.runtime_entity_id));
                let Some(proxy_id) = proxy_id else {
                    diagnostics.push(diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                        DiagnosticSeverity::Warning,
                        RenderCommandStage::Apply,
                        CommandResult::Skipped,
                        ReasonCode::RemoveMissingProxy,
                        "remove_missing_proxy",
                    ));
                    continue;
                };
                if scene.remove_proxy(proxy_id).is_none() {
                    diagnostics.push(diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                        DiagnosticSeverity::Warning,
                        RenderCommandStage::Apply,
                        CommandResult::Skipped,
                        ReasonCode::RemoveMissingProxy,
                        "remove_missing_proxy",
                    ));
                }
            }
            RenderCommandPayload::UpdateTransform { transform } => {
                let Some(proxy_id) = command
                    .proxy_id
                    .or_else(|| scene.proxy_for_runtime(command.runtime_entity_id))
                else {
                    diagnostics.push(missing_proxy_apply_diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                    ));
                    continue;
                };
                let Some(proxy) = scene.proxy_mut(proxy_id) else {
                    diagnostics.push(missing_proxy_apply_diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                    ));
                    continue;
                };
                proxy.common.previous_transform = proxy.common.transform.clone();
                proxy.common.transform = transform.clone();
                proxy.common.bounds = crate::render_state::Bounds::from_transform(transform);
                proxy.common.version += 1;
            }
            RenderCommandPayload::UpdateRenderState {
                visible,
                layer,
                descriptor,
                ..
            } => {
                let Some(proxy_id) = command
                    .proxy_id
                    .or_else(|| scene.proxy_for_runtime(command.runtime_entity_id))
                else {
                    diagnostics.push(missing_proxy_apply_diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                    ));
                    continue;
                };
                let Some(proxy) = scene.proxy_mut(proxy_id) else {
                    diagnostics.push(missing_proxy_apply_diagnostic(
                        diagnostics.len() as u64 + 1,
                        command,
                    ));
                    continue;
                };
                proxy.common.visible = *visible;
                proxy.common.layer = layer.clone();
                proxy.common.version += 1;
                if let Some(descriptor) = descriptor {
                    proxy.payload = descriptor.payload.clone();
                }
            }
            RenderCommandPayload::UpdateDynamicData | RenderCommandPayload::UpdateInstanceData => {}
        }
    }
    diagnostics
}

fn missing_proxy_apply_diagnostic(
    diagnostic_id: u64,
    command: &RenderCommand,
) -> RenderCommandDiagnostic {
    diagnostic(
        diagnostic_id,
        command,
        DiagnosticSeverity::Warning,
        RenderCommandStage::Apply,
        CommandResult::Skipped,
        ReasonCode::MissingProxy,
        "missing_proxy",
    )
}

fn diagnostic(
    diagnostic_id: u64,
    command: &RenderCommand,
    severity: DiagnosticSeverity,
    stage: RenderCommandStage,
    result: CommandResult,
    reason_code: ReasonCode,
    code: &'static str,
) -> RenderCommandDiagnostic {
    RenderCommandDiagnostic {
        diagnostic_id,
        frame_index: command.frame_index,
        severity,
        code,
        stage,
        runtime_entity_id: Some(command.runtime_entity_id),
        source_entity_id: Some(command.source_entity_id.clone()),
        proxy_id: command.proxy_id,
        command_id: Some(command.command_id),
        command_type: Some(command.command_type),
        payload_kind: Some(command.payload_kind),
        result,
        reason_code,
        trace_id: command.trace_id.clone(),
    }
}

fn lifecycle_order(command_type: RenderCommandType) -> u8 {
    match command_type {
        RenderCommandType::RemoveProxy => 0,
        RenderCommandType::AddProxy => 1,
        _ => 2,
    }
}

fn command_type_order(command_type: RenderCommandType) -> u8 {
    match command_type {
        RenderCommandType::RemoveProxy => 0,
        RenderCommandType::AddProxy => 1,
        RenderCommandType::UpdateRenderState => 2,
        RenderCommandType::UpdateTransform => 3,
        RenderCommandType::UpdateDynamicData => 4,
        RenderCommandType::UpdateInstanceData => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Renderable;
    use crate::math::Vec3;

    fn source() -> SourceEntityId {
        SourceEntityId::from("entity-a")
    }

    fn runtime() -> RuntimeEntityId {
        RuntimeEntityId::new(1, 0)
    }

    fn other_runtime() -> RuntimeEntityId {
        RuntimeEntityId::new(2, 0)
    }

    fn transform(x: f32) -> Transform {
        Transform {
            local_position: Vec3 { x, y: 0.0, z: 0.0 },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        }
    }

    fn renderable(mesh: &str) -> Renderable {
        Renderable {
            mesh_ref: Some(mesh.to_string()),
            material_ref: None,
            visible: true,
            layer: "default".to_string(),
        }
    }

    fn descriptor(mesh: &str) -> RenderProxyDescriptor {
        RenderProxyDescriptor::from_renderable(renderable(mesh))
    }

    fn command(
        id: u64,
        command_type: RenderCommandType,
        payload: RenderCommandPayload,
    ) -> RenderCommand {
        RenderCommand::new(
            RenderCommandId(id),
            1,
            source(),
            runtime(),
            None,
            command_type,
            payload,
            DirtyType::RenderState,
        )
    }

    fn command_for(
        id: u64,
        source_entity_id: SourceEntityId,
        runtime_entity_id: RuntimeEntityId,
        command_type: RenderCommandType,
        payload: RenderCommandPayload,
    ) -> RenderCommand {
        RenderCommand::new(
            RenderCommandId(id),
            1,
            source_entity_id,
            runtime_entity_id,
            None,
            command_type,
            payload,
            DirtyType::RenderState,
        )
    }

    fn existing_scene() -> RenderSceneState {
        let mut scene = RenderSceneState::new();
        apply_batch(
            &mut scene,
            &[command(
                1,
                RenderCommandType::AddProxy,
                RenderCommandPayload::AddProxy {
                    transform: transform(1.0),
                    descriptor: descriptor("mesh-a"),
                },
            )],
        );
        scene
    }

    #[test]
    fn sort_commands_deterministically_by_sort_key() {
        let update = command(
            2,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(2.0),
            },
        );
        let add = command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        );
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(update);
        buffer.push(add);

        queue.collect(vec![buffer]);
        queue.stable_sort();

        assert_eq!(
            queue.pending_commands[0].command_type,
            RenderCommandType::AddProxy
        );
        assert_eq!(
            queue.pending_commands[1].command_type,
            RenderCommandType::UpdateTransform
        );
    }

    #[test]
    fn thread_local_buffers_collect_in_stable_order() {
        let mut first = ThreadLocalCommandBuffer::new();
        first.push(command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        ));
        let mut second = ThreadLocalCommandBuffer::new();
        second.push(command(
            2,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(2.0),
            },
        ));
        let mut queue = RenderCommandQueue::new(1);

        queue.collect(vec![first, second]);

        assert_eq!(queue.pending_commands.len(), 2);
        assert_eq!(queue.pending_commands[0].command_id, RenderCommandId(1));
        assert_eq!(queue.pending_commands[1].command_id, RenderCommandId(2));
    }

    #[test]
    fn add_proxy_creates_mappings() {
        let mut scene = RenderSceneState::new();
        let commands = vec![command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        )];

        let diagnostics = apply_batch(&mut scene, &commands);

        assert!(diagnostics.is_empty());
        assert_eq!(scene.proxies_len(), 1);
        assert!(scene.proxy_for_source(&source()).is_some());
    }

    #[test]
    fn remove_proxy_removes_mappings() {
        let mut scene = RenderSceneState::new();
        apply_batch(
            &mut scene,
            &[command(
                1,
                RenderCommandType::AddProxy,
                RenderCommandPayload::AddProxy {
                    transform: transform(1.0),
                    descriptor: descriptor("mesh-a"),
                },
            )],
        );
        let remove = command(
            2,
            RenderCommandType::RemoveProxy,
            RenderCommandPayload::RemoveProxy,
        );

        let diagnostics = apply_batch(&mut scene, &[remove]);

        assert!(diagnostics.is_empty());
        assert_eq!(scene.proxies_len(), 0);
        assert!(scene.proxy_for_source(&source()).is_none());
    }

    #[test]
    fn update_transform_changes_common_transform_only() {
        let mut scene = RenderSceneState::new();
        apply_batch(
            &mut scene,
            &[command(
                1,
                RenderCommandType::AddProxy,
                RenderCommandPayload::AddProxy {
                    transform: transform(1.0),
                    descriptor: descriptor("mesh-a"),
                },
            )],
        );

        apply_batch(
            &mut scene,
            &[command(
                2,
                RenderCommandType::UpdateTransform,
                RenderCommandPayload::UpdateTransform {
                    transform: transform(5.0),
                },
            )],
        );

        let proxy_id = scene.proxy_for_source(&source()).unwrap();
        let proxy = scene.proxy(proxy_id).unwrap();
        assert_eq!(proxy.common.transform.local_position.x, 5.0);
        assert_eq!(proxy.common.previous_transform.local_position.x, 1.0);
        assert_eq!(proxy.payload.kind(), RenderPayloadKind::Mesh);
    }

    #[test]
    fn update_render_state_changes_visibility_only() {
        let mut scene = RenderSceneState::new();
        apply_batch(
            &mut scene,
            &[command(
                1,
                RenderCommandType::AddProxy,
                RenderCommandPayload::AddProxy {
                    transform: transform(1.0),
                    descriptor: descriptor("mesh-a"),
                },
            )],
        );

        apply_batch(
            &mut scene,
            &[command(
                2,
                RenderCommandType::UpdateRenderState,
                RenderCommandPayload::UpdateRenderState {
                    visible: false,
                    layer: "hidden".to_string(),
                    payload_kind: RenderPayloadKind::Mesh,
                    descriptor: None,
                },
            )],
        );

        let proxy = scene
            .proxy(scene.proxy_for_source(&source()).unwrap())
            .unwrap();
        assert!(!proxy.common.visible);
        assert_eq!(proxy.common.layer, "hidden");
        assert_eq!(proxy.common.transform.local_position.x, 1.0);
    }

    #[test]
    fn multiple_transform_updates_keep_last_value() {
        let scene = existing_scene();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            2,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(2.0),
            },
        ));
        buffer.push(command(
            3,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(3.0),
            },
        ));
        queue.collect(vec![buffer]);

        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        match &merged[0].payload {
            RenderCommandPayload::UpdateTransform { transform } => {
                assert_eq!(transform.local_position.x, 3.0);
            }
            other => panic!("expected transform update, got {other:?}"),
        }
        assert_eq!(
            queue.diagnostics[0].reason_code,
            ReasonCode::MergedLastValueWins
        );
    }

    #[test]
    fn add_then_update_merges_into_add() {
        let scene = RenderSceneState::new();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        ));
        buffer.push(command(
            2,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(5.0),
            },
        ));
        queue.collect(vec![buffer]);

        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command_type, RenderCommandType::AddProxy);
        match &merged[0].payload {
            RenderCommandPayload::AddProxy { transform, .. } => {
                assert_eq!(transform.local_position.x, 5.0);
            }
            other => panic!("expected add proxy, got {other:?}"),
        }
    }

    #[test]
    fn add_then_remove_becomes_noop() {
        let scene = RenderSceneState::new();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        ));
        buffer.push(command(
            2,
            RenderCommandType::RemoveProxy,
            RenderCommandPayload::RemoveProxy,
        ));
        queue.collect(vec![buffer]);

        let merged = queue.normalize_merge(&scene);

        assert!(merged.is_empty());
        assert_eq!(queue.diagnostics[0].reason_code, ReasonCode::CoveredByNoop);
    }

    #[test]
    fn update_missing_proxy_reports_diagnostic() {
        let scene = RenderSceneState::new();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            1,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(9.0),
            },
        ));
        queue.collect(vec![buffer]);

        let merged = queue.normalize_merge(&scene);

        assert!(merged.is_empty());
        assert_eq!(queue.diagnostics.len(), 1);
        assert_eq!(queue.diagnostics[0].reason_code, ReasonCode::MissingProxy);
    }

    #[test]
    fn remove_then_add_recreates_proxy() {
        let scene = existing_scene();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            2,
            RenderCommandType::RemoveProxy,
            RenderCommandPayload::RemoveProxy,
        ));
        buffer.push(command(
            3,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(6.0),
                descriptor: descriptor("mesh-b"),
            },
        ));
        queue.collect(vec![buffer]);

        let merged = queue.normalize_merge(&scene);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].command_type, RenderCommandType::RemoveProxy);
        assert_eq!(merged[1].command_type, RenderCommandType::AddProxy);
    }

    #[test]
    fn missing_proxy_generates_warning_event() {
        let scene = RenderSceneState::new();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            1,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(9.0),
            },
        ));
        queue.collect(vec![buffer]);
        let merged = queue.normalize_merge(&scene);

        let report = queue.build_report(RenderFrameReportLevel::Summary, &merged, &[]);

        assert_eq!(report.counters.raw_command_count, 1);
        assert_eq!(report.counters.missing_proxy_count, 1);
        assert_eq!(report.counters.warning_count, 1);
        assert_eq!(report.render_events[0].event_code, "missing_proxy");
    }

    #[test]
    fn render_command_queue_exposes_projection_summary() {
        let mut queue = RenderCommandQueue::new(7);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            1,
            RenderCommandType::AddProxy,
            RenderCommandPayload::AddProxy {
                transform: transform(1.0),
                descriptor: descriptor("mesh-a"),
            },
        ));
        queue.collect(vec![buffer]);

        let projection = queue.projection_summary();

        assert_eq!(projection.kind, ProjectionKind::Render);
        assert_eq!(projection.source_domain, ProjectionDomain::World);
        assert_eq!(projection.target_domain, ProjectionDomain::Render);
        assert_eq!(projection.projected_count, 1);
    }

    #[test]
    fn merged_commands_increment_covered_or_merged_counter() {
        let scene = existing_scene();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command(
            2,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(2.0),
            },
        ));
        buffer.push(command(
            3,
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(3.0),
            },
        ));
        queue.collect(vec![buffer]);
        let merged = queue.normalize_merge(&scene);

        let report = queue.build_report(RenderFrameReportLevel::Summary, &merged, &[]);

        assert_eq!(report.counters.raw_command_count, 2);
        assert_eq!(report.counters.merged_command_count, 1);
        assert_eq!(report.counters.covered_command_count, 1);
    }

    #[test]
    fn release_report_level_does_not_store_payload_dump() {
        let scene = existing_scene();
        let mut queue = RenderCommandQueue::new(1);
        let mut buffer = ThreadLocalCommandBuffer::new();
        buffer.push(command_for(
            2,
            SourceEntityId::from("entity-b"),
            other_runtime(),
            RenderCommandType::UpdateTransform,
            RenderCommandPayload::UpdateTransform {
                transform: transform(9.0),
            },
        ));
        queue.collect(vec![buffer]);
        let merged = queue.normalize_merge(&scene);

        let report = queue.build_report(RenderFrameReportLevel::Off, &merged, &[]);

        assert_eq!(report.report_level, RenderFrameReportLevel::Off);
        assert!(report.changed_entities.is_empty());
        assert!(report.render_events.is_empty());
        assert_eq!(report.counters.missing_proxy_count, 1);
    }
}
