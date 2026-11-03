//! Stage-specific view used by `ProjectGameDefinition::rule_typed`.

use crate::{EntityHandle, GameCallContext, ProjectDiagnosticSink, Transform, Value, WorldRead};
use std::collections::BTreeMap;

/// Rule callbacks can read the world and queue gameplay mutations. Submit media
/// and host-control intents from a fixed-update, AUI, or lifecycle callback.
///
/// ```compile_fail
/// use project_game_sdk::{RuleContext, EntityHandle};
/// fn rule(ctx: &mut RuleContext<'_, '_>, entity: &EntityHandle) {
///     ctx.mutations().audio_source(entity).play();
/// }
/// ```
/// ```compile_fail
/// use project_game_sdk::{RuleContext, EntityHandle};
/// fn rule(ctx: &mut RuleContext<'_, '_>, entity: &EntityHandle) {
///     ctx.mutations().animator2d(entity).play("idle");
/// }
/// ```
/// ```compile_fail
/// use project_game_sdk::RuleContext;
/// fn rule(ctx: &mut RuleContext<'_, '_>) { ctx.request_exit(); }
/// ```
pub struct RuleContext<'call, 'world> {
    context: &'call mut GameCallContext<'world>,
}

impl<'call, 'world> RuleContext<'call, 'world> {
    pub(crate) fn new(context: &'call mut GameCallContext<'world>) -> Self {
        Self { context }
    }

    pub fn world(&self) -> &dyn WorldRead {
        self.context.world()
    }

    pub fn diagnostics(&mut self) -> &mut ProjectDiagnosticSink {
        self.context.diagnostics()
    }

    pub fn mutations(&mut self) -> RuleMutationWriter<'_> {
        RuleMutationWriter {
            writer: self.context.mutations(),
        }
    }
}

/// A view of the existing deferred queue, restricted to Rule-stage operations.
/// No raw `extend` or conversion to the unrestricted writer is exposed.
pub struct RuleMutationWriter<'a> {
    writer: &'a mut crate::DeferredMutationWriter,
}

impl RuleMutationWriter<'_> {
    pub fn write_transform(&mut self, entity: &EntityHandle, transform: Transform) {
        self.writer.write_transform(entity, transform);
    }

    pub fn write_component_field(
        &mut self,
        entity: &EntityHandle,
        component_type: impl Into<String>,
        field_path: impl Into<String>,
        value: Value,
    ) {
        self.writer
            .write_component_field(entity, component_type, field_path, value);
    }

    pub fn replace_dynamic_component(
        &mut self,
        entity: &EntityHandle,
        component_type: impl Into<String>,
        fields: BTreeMap<String, Value>,
    ) {
        self.writer
            .replace_dynamic_component(entity, component_type, fields);
    }

    pub fn instantiate_prefab(&mut self, prefab_id: impl Into<String>) {
        self.writer.instantiate_prefab(prefab_id);
    }

    pub fn instantiate_prefab_at(&mut self, prefab_id: impl Into<String>, position: [f32; 3]) {
        self.writer.instantiate_prefab_at(prefab_id, position);
    }

    pub fn despawn(&mut self, entity: &EntityHandle) {
        self.writer.despawn(entity);
    }
}
