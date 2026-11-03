//! Per-session retained UI ownership and callback acceptance boundary.
use crate::aui_instance::{AuiDispatchError, AuiInstance, MAX_AUI_SUBSCRIPTIONS};
use crate::runtime_package::RuntimeAuiDocumentRegistry;
use project_runtime_sdk::aui::*;
use project_runtime_sdk::aui_extension::{UiExtensionPropertyUpdate, UiExtensionTypeDescriptor};
use project_runtime_sdk::aui_host::{UiInvalidateKind, UiQuery, UiQueryValue};
use project_runtime_sdk::aui_wire::{UiOwnershipUpdate, UiPropertyUpdate, UiUpdateBatch};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
pub struct AuiSession {
    animations: crate::aui_animation::AuiAnimations,
    pending_animations: Option<crate::aui_animation::AuiAnimations>,
    animation_event_sequence: u64,
    instances: BTreeMap<u64, AuiInstance>,
    documents: BTreeMap<String, u64>,
    pending: Option<BTreeMap<u64, Vec<UiPropertyUpdate>>>,
    pending_custom: Option<BTreeMap<u64, Vec<UiExtensionPropertyUpdate>>>,
    pending_ownership: Option<BTreeMap<u64, Vec<UiOwnershipUpdate>>>,
    failure: Option<UiError>,
    pending_subscriptions: Vec<PendingSubscription>,
    dispatch: Option<(u64, UiNotification)>,
    templates: BTreeMap<String, Arc<crate::aui::AuiDocument>>,
    pending_instances: BTreeMap<u64, AuiInstance>,
    pending_invalidations: Vec<(UiNodeHandle, UiInvalidateKind)>,
    custom_descriptors: BTreeMap<String, UiExtensionTypeDescriptor>,
}

impl Drop for AuiSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

enum PendingSubscription {
    Subscribe {
        token: UiSubscriptionToken,
        target: UiNodeHandle,
        filter: UiEventFilter,
        scope: UiSubscriptionScope,
        callback_id: UiCallbackId,
    },
    Cancel(UiSubscriptionToken),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuiSessionDispatchCall {
    pub callback_id: UiCallbackId,
    pub notification: UiNotification,
}

fn invalid(kind: UiErrorKind) -> UiError {
    UiError {
        kind,
        handle: None,
        property: None,
    }
}

impl AuiSession {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        self.observe_queue_memory(sample);
        self.animations.observe_memory(sample, "other", "resident");
        if let Some(pending) = &self.pending_animations {
            pending.observe_memory(sample, "queues", "queuePayload");
        }
        for instance in self.instances.values() {
            instance.observe_memory(sample, "other", "resident");
        }
        for instance in self.pending_instances.values() {
            instance.observe_memory(sample, "queues", "queuePayload");
        }
    }

    pub(crate) fn observe_queue_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        fn value(value: &UiValue, sample: &mut crate::resource_memory::ResourceMemorySample) {
            match value {
                UiValue::Text(text) => sample.cpu_string(text, "queues", &["queuePayload"]),
                UiValue::Resource(Some(resource)) => {
                    sample.cpu_string(&resource.asset_id, "queues", &["queuePayload"]);
                    sample.cpu_string(&resource.guid, "queues", &["queuePayload"]);
                }
                _ => {}
            }
        }
        sample.cpu_vec(&self.pending_subscriptions, "queues", &["queuePayload"]);
        sample.cpu_vec(&self.pending_invalidations, "queues", &["queuePayload"]);
        for updates in self.pending.iter().flat_map(|groups| groups.values()) {
            sample.cpu_vec(updates, "queues", &["queuePayload"]);
            for update in updates {
                value(&update.value, sample);
            }
        }
        for updates in self
            .pending_custom
            .iter()
            .flat_map(|groups| groups.values())
        {
            sample.cpu_vec(updates, "queues", &["queuePayload"]);
            for update in updates {
                sample.cpu_string(&update.name, "queues", &["queuePayload"]);
                match &update.value {
                    project_runtime_sdk::aui_extension::UiExtensionValue::Text(text)
                    | project_runtime_sdk::aui_extension::UiExtensionValue::Asset(text) => {
                        sample.cpu_string(text, "queues", &["queuePayload"]);
                    }
                    _ => {}
                }
            }
        }
        for updates in self
            .pending_ownership
            .iter()
            .flat_map(|groups| groups.values())
        {
            sample.cpu_vec(updates, "queues", &["queuePayload"]);
        }
        if let Some((
            _,
            UiNotification {
                kind:
                    UiNotificationKind::Changed {
                        old_value,
                        new_value,
                        ..
                    },
                ..
            },
        )) = &self.dispatch
        {
            value(old_value, sample);
            value(new_value, sample);
        }
        sample.partial(
            "cpu.owner.queues",
            "sdk.ui.property+custom+ownershipUpdates+subscriptions+invalidations+dispatch.actualBacking;stdMapNodesOpaque",
        );
    }

    pub fn advance_animations(&mut self, frame: u64, delta_us: u64) -> Result<(), UiError> {
        self.animations
            .advance(frame, delta_us, &mut self.instances)
    }

    pub fn cancel_drag_animations(
        &mut self,
        document: &str,
        interaction: &crate::aui::AuiInteractionResult,
    ) {
        for command in interaction
            .commands
            .iter()
            .filter(|command| command.command_kind == crate::aui::AuiCommandKind::DragStart)
        {
            if let Ok(handle) = self.resolve(document, &command.source_node) {
                self.animations
                    .cancel_translation(handle, &mut self.instances);
            }
        }
    }

    pub(crate) fn animation_request(
        &mut self,
        request: project_runtime_sdk::aui_animation::AnimationRequest,
    ) -> Result<project_runtime_sdk::aui_animation::AnimationReply, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let animations = self
            .pending_animations
            .get_or_insert_with(|| self.animations.clone());
        animations.request(request, &self.instances, &self.pending_instances)
    }
    pub fn publish_geometry(&mut self, output: &crate::aui::AuiRuntimePresentOutput) -> usize {
        if !output.layout.diagnostics.is_empty()
            || output.report.status == crate::aui::AuiRuntimePresentStatus::Failed
            || output.report.diagnostics.iter().any(|d| {
                matches!(
                    d.code.as_str(),
                    "aui_extension.stabilization_limit_exceeded"
                        | "aui_extension.frame_update_failed"
                )
            })
        {
            return 0;
        }
        let id = &output.resolved_document.document_id;
        self.documents
            .get(id)
            .and_then(|id| self.instances.get_mut(id))
            .map_or(0, |instance| {
                instance.publish_geometry(&output.resolved_document, &output.layout)
            })
    }
    #[cfg(test)]
    pub(crate) fn pending_is_empty_for_test(&self) -> bool {
        self.pending.is_none()
    }
    pub fn mount(documents: &RuntimeAuiDocumentRegistry) -> Result<Self, String> {
        let mut result = Self::default();
        for (id, document) in &documents.documents_by_id {
            let document = Arc::new(document.clone());
            let instance = AuiInstance::mount(Arc::clone(&document))
                .map_err(|_| format!("Cannot mount UI document {id}"))?;
            let identity = instance.identity();
            result.documents.insert(id.clone(), identity);
            result.instances.insert(identity, instance);
            result.templates.insert(id.clone(), document);
        }
        Ok(result)
    }

    pub fn document(&self, id: &str) -> Option<&AuiInstance> {
        self.documents.get(id).and_then(|id| self.instances.get(id))
    }

    /// Consumes committed UI only; staging a callback does not expose its unaccepted properties.
    pub fn layout_document(
        &mut self,
        document: &str,
        canvas: &str,
        measure: &mut dyn crate::aui_layout_runtime::AuiIntrinsicMeasure,
    ) -> Result<&crate::aui_layout_runtime::AuiLayoutSnapshot, Vec<crate::aui::AuiValidationItem>>
    {
        let instance = self
            .documents
            .get(document)
            .and_then(|id| self.instances.get_mut(id));
        match instance {
            Some(instance) => instance.layout(canvas, measure),
            None => Err(vec![crate::aui::AuiValidationItem::error(
                "aui_session_document_missing",
                None,
                None,
                format!("Unknown UI document {document}"),
                "Use a mounted RuntimePackage document ID.",
            )]),
        }
    }

    pub(crate) fn begin(&mut self) {
        self.pending_animations = None;
        self.pending = None;
        self.pending_custom = None;
        self.pending_ownership = None;
        self.failure = None;
        self.pending_subscriptions.clear();
        self.pending_instances.clear();
        self.pending_invalidations.clear();
    }

    pub(crate) fn failed(&self) -> bool {
        self.failure.is_some()
    }

    pub(crate) fn reject(&mut self, error: UiError) -> UiError {
        self.pending_animations = None;
        self.pending = None;
        self.pending_custom = None;
        self.pending_ownership = None;
        self.pending_instances.clear();
        self.pending_invalidations.clear();
        *self.failure.get_or_insert(error)
    }

    pub(crate) fn query(&self, query: UiQuery) -> Result<UiQueryValue, UiError> {
        match query {
            UiQuery::Animation { .. } => Err(invalid(UiErrorKind::InvalidValue)),
            UiQuery::Resolve { document, node } => {
                self.resolve(&document, &node).map(UiQueryValue::Handle)
            }
            UiQuery::NodeType { handle } => self
                .effective_instance(handle.instance)?
                .node_type(handle)
                .map(UiQueryValue::NodeType),
            UiQuery::Read { handle, property } => {
                let value = self
                    .effective_instance(handle.instance)?
                    .read(handle, property)?;
                if matches!(&value, UiValue::Text(text) if text.len() > project_runtime_sdk::aui_wire::MAX_UI_UPDATE_TEXT_BYTES)
                {
                    return Err(invalid(UiErrorKind::CapacityExceeded));
                }
                Ok(UiQueryValue::Value(value))
            }
            UiQuery::LocalToScreen { handle, point } => self
                .effective_instance(handle.instance)?
                .local_to_screen(handle, point)
                .map(UiQueryValue::Point),
            UiQuery::ScreenToLocal { handle, point } => self
                .effective_instance(handle.instance)?
                .screen_to_local(handle, point)
                .map(UiQueryValue::Point),
        }
    }

    pub(crate) fn stage(&mut self, batch: UiUpdateBatch) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.pending.is_some()
            || self.pending_custom.is_some()
            || self.pending_ownership.is_some()
        {
            return Err(self.reject(invalid(UiErrorKind::InvalidValue)));
        }
        let mut groups: BTreeMap<u64, Vec<UiPropertyUpdate>> = BTreeMap::new();
        for update in batch.updates() {
            groups
                .entry(update.handle.instance)
                .or_default()
                .push(update.clone());
        }
        let mut custom_groups: BTreeMap<u64, Vec<UiExtensionPropertyUpdate>> = BTreeMap::new();
        for update in batch.custom_updates() {
            custom_groups
                .entry(update.handle.instance)
                .or_default()
                .push(update.clone());
        }
        let mut ownership_groups: BTreeMap<u64, Vec<UiOwnershipUpdate>> = BTreeMap::new();
        for update in batch.ownership_updates() {
            ownership_groups
                .entry(update.handle.instance)
                .or_default()
                .push(*update);
        }
        let validated = (|| {
            let ids = groups
                .keys()
                .chain(custom_groups.keys())
                .chain(ownership_groups.keys())
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            for id in ids {
                let is_draft = self.pending_instances.contains_key(&id);
                let instance = if let Some(instance) = self.pending_instances.get_mut(&id) {
                    instance
                } else {
                    self.instances
                        .get_mut(&id)
                        .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                };
                let mut edit = instance.edit();
                for update in ownership_groups.get(&id).into_iter().flatten() {
                    edit.transfer_property_ownership(update.handle, update.property, update.owner)?;
                }
                for update in groups.get(&id).into_iter().flatten() {
                    if update.notify {
                        edit.write(update.handle, update.property, update.value.clone())?;
                    } else {
                        edit.write_without_notify(
                            update.handle,
                            update.property,
                            update.value.clone(),
                        )?;
                    }
                }
                if is_draft {
                    edit.commit()?;
                } else {
                    edit.validate()?;
                }
                for update in custom_groups.get(&id).into_iter().flatten() {
                    let spec = instance
                        .source()
                        .nodes
                        .get(update.handle.slot as usize)
                        .and_then(|node| node.custom.as_ref())
                        .ok_or_else(|| invalid(UiErrorKind::WrongNodeType))?;
                    let descriptor = self
                        .custom_descriptors
                        .get(&spec.type_id)
                        .ok_or_else(|| invalid(UiErrorKind::Unavailable))?;
                    if is_draft {
                        instance.apply_custom_property(
                            update.handle,
                            &update.name,
                            update.value.clone(),
                            descriptor,
                        )?;
                    } else {
                        instance.validate_custom_property(
                            update.handle,
                            &update.name,
                            &update.value,
                            descriptor,
                        )?;
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = validated {
            return Err(self.reject(error));
        }
        groups.retain(|id, _| !self.pending_instances.contains_key(id));
        custom_groups.retain(|id, _| !self.pending_instances.contains_key(id));
        ownership_groups.retain(|id, _| !self.pending_instances.contains_key(id));
        self.pending = (!groups.is_empty()).then_some(groups);
        self.pending_custom = (!custom_groups.is_empty()).then_some(custom_groups);
        self.pending_ownership = (!ownership_groups.is_empty()).then_some(ownership_groups);
        Ok(())
    }

    pub(crate) fn structure(
        &mut self,
        transaction: UiStructureTransaction,
    ) -> Result<UiStructureResult, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if transaction.version != UiStructureTransaction::VERSION
            || transaction.operations.len()
                > project_runtime_sdk::aui_host::MAX_UI_STRUCTURE_OPERATIONS
        {
            return Err(self.reject(invalid(UiErrorKind::InvalidValue)));
        }
        let touched = transaction
            .operations
            .iter()
            .map(|operation| match operation {
                UiStructureOperation::Instantiate { parent, .. } => parent.instance,
                UiStructureOperation::Delete { target }
                | UiStructureOperation::Reparent { target, .. }
                | UiStructureOperation::SetSiblingIndex { target, .. } => target.instance,
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut working = BTreeMap::new();
        for id in &touched {
            let Some(source) = self
                .pending_instances
                .get(id)
                .or_else(|| self.instances.get(id))
            else {
                return Err(self.reject(invalid(UiErrorKind::InvalidHandle)));
            };
            let mut draft = source.transaction_draft();
            let ownership_updates = self
                .pending_ownership
                .as_ref()
                .and_then(|pending| pending.get(id));
            let property_updates = self.pending.as_ref().and_then(|pending| pending.get(id));
            if ownership_updates.is_some() || property_updates.is_some() {
                let mut edit = draft.edit();
                for update in ownership_updates.into_iter().flatten() {
                    edit.transfer_property_ownership(update.handle, update.property, update.owner)?;
                }
                for update in property_updates.into_iter().flatten() {
                    if update.notify {
                        edit.write(update.handle, update.property, update.value.clone())?;
                    } else {
                        edit.write_without_notify(
                            update.handle,
                            update.property,
                            update.value.clone(),
                        )?;
                    }
                }
                edit.commit()?;
            }
            if let Some(updates) = self
                .pending_custom
                .as_ref()
                .and_then(|pending| pending.get(id))
            {
                for update in updates {
                    let spec = draft
                        .source()
                        .nodes
                        .get(update.handle.slot as usize)
                        .and_then(|node| node.custom.as_ref())
                        .ok_or_else(|| invalid(UiErrorKind::WrongNodeType))?;
                    let descriptor = self
                        .custom_descriptors
                        .get(&spec.type_id)
                        .ok_or_else(|| invalid(UiErrorKind::Unavailable))?;
                    draft.apply_custom_property(
                        update.handle,
                        &update.name,
                        update.value.clone(),
                        descriptor,
                    )?;
                }
            }
            working.insert(*id, draft);
        }
        let applied = (|| {
            let mut result = UiStructureResult::default();
            for operation in transaction.operations {
                match operation {
                    UiStructureOperation::Instantiate {
                        template,
                        parent,
                        sibling_index,
                    } => {
                        let template = self
                            .templates
                            .get(&template)
                            .ok_or_else(|| invalid(UiErrorKind::TemplateMissing))?;
                        let instance = working
                            .get_mut(&parent.instance)
                            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?;
                        result.instantiated.push(instance.instantiate_template(
                            template,
                            parent,
                            sibling_index,
                        )?);
                    }
                    UiStructureOperation::Delete { target } => working
                        .get_mut(&target.instance)
                        .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                        .delete_subtree(target)?,
                    UiStructureOperation::Reparent {
                        target,
                        new_parent,
                        sibling_index,
                        mode,
                    } => {
                        if target.instance != new_parent.instance {
                            return Err(invalid(UiErrorKind::IncompatibleRoot));
                        }
                        working
                            .get_mut(&target.instance)
                            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                            .reparent(target, new_parent, sibling_index, mode)?;
                    }
                    UiStructureOperation::SetSiblingIndex {
                        target,
                        sibling_index,
                    } => working
                        .get_mut(&target.instance)
                        .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
                        .set_sibling_index(target, sibling_index)?,
                }
            }
            Ok(result)
        })();
        let result = match applied {
            Ok(result) => result,
            Err(error) => return Err(self.reject(error)),
        };
        let output_size = serde_json::to_vec(&Result::<&UiStructureResult, UiError>::Ok(&result))
            .map_err(|_| self.reject(invalid(UiErrorKind::InvalidValue)))?
            .len();
        if output_size > project_runtime_sdk::aui_host::MAX_UI_STRUCTURE_WIRE_BYTES {
            return Err(self.reject(invalid(UiErrorKind::CapacityExceeded)));
        }
        for (id, draft) in working {
            self.pending_instances.insert(id, draft);
        }
        if let Some(pending) = &mut self.pending {
            for id in &touched {
                pending.remove(id);
            }
            if pending.is_empty() {
                self.pending = None;
            }
        }
        if let Some(pending) = &mut self.pending_custom {
            for id in &touched {
                pending.remove(id);
            }
            if pending.is_empty() {
                self.pending_custom = None;
            }
        }
        if let Some(pending) = &mut self.pending_ownership {
            for id in &touched {
                pending.remove(id);
            }
            if pending.is_empty() {
                self.pending_ownership = None;
            }
        }
        Ok(result)
    }

    fn effective_instance(&self, id: u64) -> Result<&AuiInstance, UiError> {
        self.pending_instances
            .get(&id)
            .or_else(|| self.instances.get(&id))
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))
    }

    pub(crate) fn subscribe(
        &mut self,
        target: UiNodeHandle,
        filter: UiEventFilter,
        scope: UiSubscriptionScope,
        callback_id: UiCallbackId,
    ) -> Result<UiSubscriptionToken, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if callback_id.0 == 0 {
            return Err(self.reject(invalid(UiErrorKind::InvalidValue)));
        }
        let pending_count = self
            .pending_subscriptions
            .iter()
            .filter(|operation| matches!(operation, PendingSubscription::Subscribe { .. }))
            .count();
        let active_count = self
            .instances
            .values()
            .map(AuiInstance::subscription_count)
            .sum::<usize>();
        if active_count.saturating_add(pending_count) >= MAX_AUI_SUBSCRIPTIONS {
            return Err(self.reject(invalid(UiErrorKind::CapacityExceeded)));
        }
        let token = match self
            .pending_instances
            .get_mut(&target.instance)
            .or_else(|| self.instances.get_mut(&target.instance))
        {
            Some(instance) => match instance.reserve_subscription(target) {
                Ok(token) => token,
                Err(error) => return Err(self.reject(error)),
            },
            None => return Err(self.reject(invalid(UiErrorKind::InvalidHandle))),
        };
        self.pending_subscriptions
            .push(PendingSubscription::Subscribe {
                token,
                target,
                filter,
                scope,
                callback_id,
            });
        Ok(token)
    }

    pub(crate) fn cancel(&mut self, token: UiSubscriptionToken) -> Result<bool, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let Some(instance) = self
            .pending_instances
            .get(&token.instance)
            .or_else(|| self.instances.get(&token.instance))
        else {
            return Err(self.reject(invalid(UiErrorKind::InvalidHandle)));
        };
        let pending = self.pending_subscriptions.iter().any(|operation| {
            matches!(operation, PendingSubscription::Subscribe { token: pending, .. } if *pending == token)
        });
        if !pending && !instance.contains_subscription(token) {
            return Ok(false);
        }
        self.pending_subscriptions
            .push(PendingSubscription::Cancel(token));
        Ok(true)
    }

    pub(crate) fn finish(&mut self, accepted: bool) {
        let pending_animations = self.pending_animations.take();
        let pending = self.pending.take();
        let pending_custom = self.pending_custom.take();
        let mut pending_ownership = self.pending_ownership.take().unwrap_or_default();
        let pending_instances = std::mem::take(&mut self.pending_instances);
        let animations_changed = pending_animations.is_some() || !pending_instances.is_empty();
        let pending_subscriptions = std::mem::take(&mut self.pending_subscriptions);
        let pending_invalidations = std::mem::take(&mut self.pending_invalidations);
        if accepted && self.failure.is_none() {
            for (id, updates) in pending.into_iter().flatten() {
                // No mutable access to instances is exposed between stage and finish. Validation
                // therefore remains valid for every document, including ownership and generations.
                let mut edit = self
                    .instances
                    .get_mut(&id)
                    .expect("validated UI instance")
                    .edit();
                for update in pending_ownership.remove(&id).into_iter().flatten() {
                    edit.transfer_property_ownership(update.handle, update.property, update.owner)
                        .expect("validated UI property ownership");
                }
                for update in updates {
                    if update.notify {
                        edit.write(update.handle, update.property, update.value)
                            .expect("validated UI property");
                    } else {
                        edit.write_without_notify(update.handle, update.property, update.value)
                            .expect("validated UI property");
                    }
                }
                edit.commit().expect("validated UI segment");
            }
            for (id, updates) in pending_ownership {
                let mut edit = self
                    .instances
                    .get_mut(&id)
                    .expect("validated UI ownership instance")
                    .edit();
                for update in updates {
                    edit.transfer_property_ownership(update.handle, update.property, update.owner)
                        .expect("validated UI property ownership");
                }
                edit.commit().expect("validated UI ownership segment");
            }
            for (id, updates) in pending_custom.into_iter().flatten() {
                let instance = self
                    .instances
                    .get_mut(&id)
                    .expect("validated custom UI instance");
                for update in updates {
                    let spec = instance
                        .source()
                        .nodes
                        .get(update.handle.slot as usize)
                        .and_then(|node| node.custom.as_ref())
                        .expect("validated custom UI node");
                    let descriptor = self
                        .custom_descriptors
                        .get(&spec.type_id)
                        .expect("validated custom UI descriptor");
                    instance
                        .apply_custom_property(
                            update.handle,
                            &update.name,
                            update.value,
                            descriptor,
                        )
                        .expect("validated custom UI property");
                }
            }
            for (id, mut instance) in pending_instances {
                if let Some(previous) = self.instances.get_mut(&id) {
                    instance.retain_layouts_from(previous);
                }
                self.instances.insert(id, instance);
            }
            for operation in pending_subscriptions {
                match operation {
                    PendingSubscription::Subscribe {
                        token,
                        target,
                        filter,
                        scope,
                        callback_id,
                    } => self
                        .instances
                        .get_mut(&token.instance)
                        .expect("reserved UI subscription instance")
                        .subscribe_reserved(token, target, filter, scope, callback_id.0)
                        .expect("reserved UI subscription"),
                    PendingSubscription::Cancel(token) => {
                        if let Some(instance) = self.instances.get_mut(&token.instance) {
                            instance.cancel_subscription(token);
                        }
                    }
                }
            }
            for (handle, kind) in pending_invalidations {
                self.instances
                    .get_mut(&handle.instance)
                    .expect("validated custom UI instance")
                    .invalidate_custom(handle, kind)
                    .expect("validated custom UI invalidation");
            }
            if let Some(mut animations) = pending_animations {
                animations.capture_new_stamps(&self.animations, &self.instances);
                self.animations = animations;
            }
            if animations_changed {
                self.animations
                    .resolve(&mut self.instances)
                    .expect("validated animation values");
            }
        }
        self.failure = None;
    }

    pub(crate) fn next_dispatch_call(
        &mut self,
    ) -> Result<Option<AuiSessionDispatchCall>, AuiDispatchError> {
        if let Some(event) = self.animations.events.pop_front() {
            self.animation_event_sequence = self
                .animation_event_sequence
                .checked_add(1)
                .expect("animation event sequence exhausted");
            return Ok(Some(AuiSessionDispatchCall {
                callback_id: event.callback_id,
                notification: UiNotification {
                    sequence: self.animation_event_sequence,
                    target: event.target,
                    kind: UiNotificationKind::Animation {
                        handle: event.handle,
                        outcome: event.outcome,
                    },
                },
            }));
        }
        loop {
            if let Some((instance_id, notification)) = self.dispatch.clone() {
                let instance = self
                    .instances
                    .get_mut(&instance_id)
                    .expect("dispatch instance remains mounted");
                if let Some(listener) = instance.next_dispatch_listener()? {
                    return Ok(Some(AuiSessionDispatchCall {
                        callback_id: UiCallbackId(listener.callback_id),
                        notification,
                    }));
                }
                instance.end_dispatch()?;
                self.dispatch = None;
                continue;
            }
            let ids = self.instances.keys().copied().collect::<Vec<_>>();
            let mut started = false;
            for id in ids {
                let instance = self.instances.get_mut(&id).expect("listed UI instance");
                if let Some(notification) = instance.begin_dispatch()? {
                    self.dispatch = Some((id, notification));
                    started = true;
                    break;
                }
            }
            if !started {
                return Ok(None);
            }
        }
    }

    pub(crate) fn shutdown(&mut self) {
        self.pending_animations = None;
        self.animations = Default::default();
        self.pending = None;
        self.pending_custom = None;
        self.pending_ownership = None;
        self.pending_instances.clear();
        self.pending_subscriptions.clear();
        self.pending_invalidations.clear();
        self.custom_descriptors.clear();
        self.dispatch = None;
        for instance in self.instances.values_mut() {
            instance.clear_subscriptions();
            instance.dispose();
        }
        self.documents.clear();
        self.templates.clear();
        self.instances.clear();
        self.failure = None;
    }

    pub(crate) fn stage_custom_invalidation(
        &mut self,
        target: UiNodeHandle,
        kind: UiInvalidateKind,
    ) -> Result<(), UiError> {
        let instance = self.effective_instance(target.instance)?;
        if !instance.is_custom_node(target) {
            return Err(self.reject(UiError {
                kind: UiErrorKind::WrongNodeType,
                handle: Some(target),
                property: None,
            }));
        }
        if !self.pending_invalidations.contains(&(target, kind)) {
            self.pending_invalidations.push((target, kind));
        }
        Ok(())
    }

    pub(crate) fn custom_work(
        &self,
    ) -> Vec<(
        crate::aui_extension::AuiCustomRuntimeNode,
        crate::aui_extension::AuiCustomDirty,
    )> {
        self.instances
            .values()
            .flat_map(AuiInstance::custom_work)
            .collect()
    }

    pub(crate) fn has_custom_work(&self) -> bool {
        self.instances.values().any(AuiInstance::has_custom_work)
    }

    pub(crate) fn custom_runtime_node(
        &self,
        handle: UiNodeHandle,
    ) -> Result<
        (
            crate::aui_extension::AuiCustomRuntimeNode,
            crate::aui_extension::AuiCustomDirty,
        ),
        UiError,
    > {
        self.instances
            .get(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .custom_runtime_node(handle)
    }

    pub(crate) fn set_custom_descriptors(
        &mut self,
        descriptors: BTreeMap<String, UiExtensionTypeDescriptor>,
    ) {
        self.custom_descriptors = descriptors;
    }

    pub(crate) fn install_custom_output(
        &mut self,
        handle: UiNodeHandle,
        output: project_runtime_sdk::aui_extension::UiExtensionOutput,
    ) -> Result<(), UiError> {
        self.instances
            .get_mut(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .install_custom_output(handle, output)
    }

    pub(crate) fn install_custom_output_with_identity(
        &mut self,
        handle: UiNodeHandle,
        output: project_runtime_sdk::aui_extension::UiExtensionOutput,
        identity: crate::aui_extension::AuiCustomStageIdentity,
    ) -> Result<bool, UiError> {
        self.instances
            .get_mut(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .install_custom_output_with_identity(handle, output, Some(identity))
    }

    pub(crate) fn complete_custom_stage(
        &mut self,
        handle: UiNodeHandle,
        stage: project_runtime_sdk::aui_extension::UiExtensionInvalidation,
    ) -> Result<(), UiError> {
        self.instances
            .get_mut(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .complete_custom_stage(handle, stage)
    }

    pub(crate) fn discard_stale_custom_output(
        &mut self,
        handle: UiNodeHandle,
        stage: project_runtime_sdk::aui_extension::UiExtensionInvalidation,
        identity: &crate::aui_extension::AuiCustomStageIdentity,
    ) -> Result<bool, UiError> {
        self.instances
            .get_mut(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .discard_stale_custom_output(handle, stage, identity)
    }

    #[cfg(test)]
    pub(crate) fn subscription_count_for_test(&self) -> usize {
        self.instances
            .values()
            .map(AuiInstance::subscription_count)
            .sum()
    }
}

impl UiAccess for AuiSession {
    fn resolve(&self, document: &str, node: &str) -> Result<UiNodeHandle, UiError> {
        self.document(document)
            .ok_or_else(|| invalid(UiErrorKind::UnknownNode))?
            .node(node)
    }
}

impl UiRead for AuiSession {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
        self.instances
            .get(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .node_type(handle)
    }
    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
        self.instances
            .get(&handle.instance)
            .ok_or_else(|| invalid(UiErrorKind::InvalidHandle))?
            .read(handle, property)
    }
}

impl UiGeometryAccess for AuiSession {
    fn local_to_screen(&self, handle: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.effective_instance(handle.instance)?
            .local_to_screen(handle, point)
    }

    fn screen_to_local(&self, handle: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.effective_instance(handle.instance)?
            .screen_to_local(handle, point)
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::resource_memory::{memory_totals, ResourceMemorySample};
    use project_runtime_sdk::aui_extension::UiExtensionValue;

    fn vector<T>(value: &Vec<T>, expected: &mut Vec<(String, u64)>) {
        if value.capacity() != 0 {
            expected.push((
                format!("cpu-storage:{:x}", value.as_ptr() as usize),
                (value.capacity() * std::mem::size_of::<T>()) as u64,
            ));
        }
    }

    #[test]
    fn ui_property_queue_capacity_and_dispatch_backing_are_observed_without_retention() {
        for capacity in [256, 4096] {
            for populated in [false, true] {
                let mut expected = Vec::new();
                let mut text = || {
                    let mut result = String::with_capacity(capacity);
                    if populated {
                        result.push_str("ui-payload");
                    }
                    expected.push((
                        format!("cpu-storage:{:x}", result.as_ptr() as usize),
                        result.capacity() as u64,
                    ));
                    result
                };
                let handle = UiNodeHandle {
                    instance: 1,
                    slot: 0,
                    generation: 1,
                };
                let mut properties = Vec::with_capacity(8);
                properties.push(UiPropertyUpdate {
                    handle,
                    property: UiProperty::Text,
                    value: UiValue::Text(text()),
                    notify: false,
                });
                properties.push(UiPropertyUpdate {
                    handle,
                    property: UiProperty::Resource,
                    value: UiValue::Resource(Some(UiResourceRef {
                        asset_id: text(),
                        guid: text(),
                    })),
                    notify: false,
                });
                let mut custom = Vec::with_capacity(8);
                custom.push(UiExtensionPropertyUpdate {
                    handle,
                    name: text(),
                    value: UiExtensionValue::Text(text()),
                });
                custom.push(UiExtensionPropertyUpdate {
                    handle,
                    name: text(),
                    value: UiExtensionValue::Asset(text()),
                });
                let dispatch = UiNotification {
                    sequence: 1,
                    target: handle,
                    kind: UiNotificationKind::Changed {
                        property: UiProperty::Text,
                        old_value: UiValue::Text(text()),
                        new_value: UiValue::Resource(Some(UiResourceRef {
                            asset_id: text(),
                            guid: text(),
                        })),
                        origin: UiChangeOrigin::Programmatic,
                    },
                };
                let mut ownership = Vec::with_capacity(8);
                ownership.push(UiOwnershipUpdate {
                    handle,
                    property: UiProperty::Text,
                    owner: UiPropertyOwner::Controller,
                });
                let mut session = AuiSession::default();
                session.pending = Some(BTreeMap::from([(1, properties)]));
                session.pending_custom = Some(BTreeMap::from([(1, custom)]));
                session.pending_ownership = Some(BTreeMap::from([(1, ownership)]));
                session.pending_subscriptions = Vec::with_capacity(8);
                session.pending_invalidations = Vec::with_capacity(16);
                session.dispatch = Some((1, dispatch));
                vector(&session.pending.as_ref().unwrap()[&1], &mut expected);
                vector(&session.pending_custom.as_ref().unwrap()[&1], &mut expected);
                vector(
                    &session.pending_ownership.as_ref().unwrap()[&1],
                    &mut expected,
                );
                vector(&session.pending_subscriptions, &mut expected);
                vector(&session.pending_invalidations, &mut expected);
                let mut sample = ResourceMemorySample::new(1, 1, 0);
                session.observe_queue_memory(&mut sample);
                for (id, bytes) in &expected {
                    let row = sample
                        .allocations
                        .iter()
                        .find(|row| &row.allocation_id == id)
                        .unwrap_or_else(|| panic!("missing UI queue backing {id} ({bytes})"));
                    assert_eq!(row.bytes, *bytes);
                    assert_eq!(row.owner, "queues");
                    assert_eq!(row.roles, ["queuePayload".to_string()].into());
                }
                let total: u64 = expected.iter().map(|(_, bytes)| bytes).sum();
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    total
                );
                session.observe_queue_memory(&mut sample);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    total
                );

                session.reject(invalid(UiErrorKind::InvalidValue));
                let mut rejected = ResourceMemorySample::new(2, 1, 0);
                session.observe_queue_memory(&mut rejected);
                assert!(memory_totals(&rejected.allocations).unwrap().cpu_allocated < total);
                // finish drops transaction Vecs; shutdown releases the current callback.
                session.finish(false);
                session.shutdown();
                let mut after = ResourceMemorySample::new(3, 1, 0);
                session.observe_queue_memory(&mut after);
                assert!(after.allocations.is_empty());
            }
        }
    }
}

#[cfg(test)]
mod retained_layout_tests {
    use super::*;
    use crate::aui::*;
    use crate::aui_layout_runtime::*;
    use crate::aui_layout_schema::*;
    use project_runtime_sdk::aui_wire::{
        UiOwnershipUpdate, UiPropertyUpdate, UiUpdateBatch, UiWriteBuffer,
    };

    struct Measure;
    impl AuiIntrinsicMeasure for Measure {
        fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
            AuiMeasureOutput {
                width: input.text.unwrap_or("").chars().count() as f32 * 10.,
                height: 20.,
                first_baseline: None,
            }
        }
    }

    #[test]
    fn slider_typed_value_is_transactional_binding_aware_and_presented() {
        let mut registry = RuntimeAuiDocumentRegistry::empty("slider-typed-value");
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children = vec!["speed".into()];
        let mut slider = AuiNode::new(
            "speed",
            AuiNodeKind::Slider,
            AuiRect::fixed_position(0., 0., 200., 24.),
        )
        .with_slider(0.5, 0.5, 2.0, 0.25);
        slider.parent = Some("root".into());
        slider.binding_refs.push(AuiBindingRef::new(
            "speed.value",
            AuiBindingTarget::SliderValue,
            "settings.speed",
            None,
        ));
        let mut document = AuiDocument::new(
            "settings",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, slider],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        registry.documents_by_id.insert("settings".into(), document);
        let mut session = AuiSession::mount(&registry).unwrap();
        let handle = session.resolve("settings", "speed").unwrap();
        assert_eq!(session.node_type(handle).unwrap(), UiNodeType::Slider);

        session.begin();
        let batch = {
            let mut ui = UiWriteBuffer::new(&session);
            let speed = ui.slider("settings", "speed").unwrap();
            speed
                .node()
                .take_property_ownership(&mut ui, UiProperty::Value)
                .unwrap();
            speed.set_value_without_notify(&mut ui, 1.37).unwrap();
            ui.finish().unwrap()
        };
        session.stage(batch).unwrap();
        session.finish(false);
        assert_eq!(
            session.read(handle, UiProperty::Value).unwrap(),
            UiValue::Number(0.5)
        );

        session.begin();
        let batch = {
            let mut ui = UiWriteBuffer::new(&session);
            let speed = ui.slider("settings", "speed").unwrap();
            speed
                .node()
                .take_property_ownership(&mut ui, UiProperty::Value)
                .unwrap();
            speed.set_value_without_notify(&mut ui, 1.37).unwrap();
            ui.finish().unwrap()
        };
        session.stage(batch).unwrap();
        session.finish(true);
        assert_eq!(
            session.read(handle, UiProperty::Value).unwrap(),
            UiValue::Number(1.25)
        );

        let mut presentation = AuiC2Runtime::default();
        assert!(presentation.observe_retained(session.document("settings")));
        assert!(session
            .document("settings")
            .unwrap()
            .presentation_properties()
            .any(|(node, property, value)| {
                node == "speed" && property == UiProperty::Value && value == UiValue::Number(1.25)
            }));
    }

    #[test]
    fn controller_takeover_preserves_an_equal_initial_value() {
        let mut registry = RuntimeAuiDocumentRegistry::empty("equal-slider-takeover");
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children = vec!["speed".into()];
        let mut slider = AuiNode::new(
            "speed",
            AuiNodeKind::Slider,
            AuiRect::fixed_position(0., 0., 200., 24.),
        )
        .with_slider(0.5, 0.5, 2.0, 0.25);
        slider.parent = Some("root".into());
        slider.binding_refs.push(AuiBindingRef::new(
            "speed.value",
            AuiBindingTarget::SliderValue,
            "settings.speed",
            None,
        ));
        let mut document = AuiDocument::new(
            "settings",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, slider],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        registry.documents_by_id.insert("settings".into(), document);
        let mut session = AuiSession::mount(&registry).unwrap();
        let handle = session.resolve("settings", "speed").unwrap();

        session.begin();
        let batch = {
            let mut ui = UiWriteBuffer::new(&session);
            let speed = ui.slider("settings", "speed").unwrap();
            speed
                .node()
                .take_property_ownership(&mut ui, UiProperty::Value)
                .unwrap();
            speed.set_value_without_notify(&mut ui, 0.5).unwrap();
            ui.finish().unwrap()
        };
        session.stage(batch).unwrap();
        session.finish(true);

        session.begin();
        session
            .stage(
                UiUpdateBatch::new(vec![UiPropertyUpdate {
                    handle,
                    property: UiProperty::Value,
                    value: UiValue::Number(1.0),
                    notify: false,
                }])
                .unwrap(),
            )
            .unwrap();
        session.finish(true);
        assert_eq!(
            session.read(handle, UiProperty::Value).unwrap(),
            UiValue::Number(1.0)
        );
    }

    #[test]
    fn binding_ownership_transfer_obeys_the_session_acceptance_boundary() {
        let mut registry = RuntimeAuiDocumentRegistry::empty("binding-ownership");
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children = vec!["label".into()];
        let mut label = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 20., 20.),
        );
        label.parent = Some("root".into());
        label.text = Some("人".into());
        label.binding_refs.push(AuiBindingRef::new(
            "label.text",
            AuiBindingTarget::TextText,
            "game.label",
            None,
        ));
        let mut document = AuiDocument::new(
            "ui",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, label],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        registry.documents_by_id.insert("ui".into(), document);
        let mut session = AuiSession::mount(&registry).unwrap();
        let handle = session.resolve("ui", "label").unwrap();

        let takeover = || {
            UiUpdateBatch::new_with_custom_and_ownership(
                vec![UiPropertyUpdate {
                    handle,
                    property: UiProperty::Text,
                    value: UiValue::Text("骑兵".into()),
                    notify: false,
                }],
                Vec::new(),
                vec![UiOwnershipUpdate {
                    handle,
                    property: UiProperty::Text,
                    owner: UiPropertyOwner::Controller,
                }],
            )
            .unwrap()
        };
        session.begin();
        session.stage(takeover()).unwrap();
        session.finish(false);
        assert_eq!(
            session.read(handle, UiProperty::Text).unwrap(),
            UiValue::Text("人".into())
        );

        session.begin();
        session.stage(takeover()).unwrap();
        session.finish(true);
        assert_eq!(
            session.read(handle, UiProperty::Text).unwrap(),
            UiValue::Text("骑兵".into())
        );

        let release = || {
            UiUpdateBatch::new_with_custom_and_ownership(
                Vec::new(),
                Vec::new(),
                vec![UiOwnershipUpdate {
                    handle,
                    property: UiProperty::Text,
                    owner: UiPropertyOwner::Binding,
                }],
            )
            .unwrap()
        };
        session.begin();
        session.stage(release()).unwrap();
        let root = session.resolve("ui", "root").unwrap();
        assert!(session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Delete { target: root },
            ]))
            .is_err());
        session.finish(false);
        session.begin();
        session
            .stage(
                UiUpdateBatch::new(vec![UiPropertyUpdate {
                    handle,
                    property: UiProperty::Text,
                    value: UiValue::Text("仍由控制器".into()),
                    notify: false,
                }])
                .unwrap(),
            )
            .unwrap();
        session.finish(true);

        session.begin();
        session.stage(release()).unwrap();
        session.finish(true);
        session.begin();
        assert_eq!(
            session
                .stage(
                    UiUpdateBatch::new(vec![UiPropertyUpdate {
                        handle,
                        property: UiProperty::Text,
                        value: UiValue::Text("冲突".into()),
                        notify: false,
                    }])
                    .unwrap()
                )
                .unwrap_err()
                .kind,
            UiErrorKind::BindingOwned
        );
        session.finish(false);
    }

    #[test]
    fn session_layout_only_consumes_accepted_callback_properties() {
        let mut registry = RuntimeAuiDocumentRegistry::empty("retained-layout");
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.layout = Some(LayoutSpec {
            mode: LayoutMode::Row,
            ..Default::default()
        });
        root.children = vec!["label".into()];
        let mut label = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 20., 20.),
        );
        label.parent = Some("root".into());
        label.text = Some("人".into());
        label.slot = Some(SlotSpec {
            width: Dimension::Content,
            height: Dimension::Content,
            ..Default::default()
        });
        let mut document = AuiDocument::new(
            "ui",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, label],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        registry.documents_by_id.insert("ui".into(), document);
        let mut session = AuiSession::mount(&registry).unwrap();
        let mut measure = Measure;
        assert!(session
            .layout_document("missing", "main", &mut measure)
            .is_err());
        assert_eq!(
            session
                .layout_document("ui", "main", &mut measure)
                .unwrap()
                .node("label")
                .unwrap()
                .rect
                .width,
            10.
        );
        let mut presentation = AuiC2Runtime::default();
        assert!(presentation.observe_retained(session.document("ui")));
        for accepted in [false, true] {
            session.begin();
            let batch = {
                let mut ui = UiWriteBuffer::new(&session);
                ui.text("ui", "label")
                    .unwrap()
                    .set_text(&mut ui, "骑兵")
                    .unwrap();
                ui.finish().unwrap()
            };
            session.stage(batch).unwrap();
            assert!(
                !presentation.observe_retained(session.document("ui")),
                "staging must not invalidate presentation"
            );
            assert_eq!(
                session
                    .layout_document("ui", "main", &mut measure)
                    .unwrap()
                    .node("label")
                    .unwrap()
                    .rect
                    .width,
                10.
            );
            session.finish(accepted);
            assert_eq!(
                presentation.observe_retained(session.document("ui")),
                accepted
            );
            assert_eq!(
                session
                    .layout_document("ui", "main", &mut measure)
                    .unwrap()
                    .node("label")
                    .unwrap()
                    .rect
                    .width,
                if accepted { 20. } else { 10. }
            );
        }
    }

    #[test]
    fn subscription_changes_follow_callback_acceptance_and_shutdown_clears_them() {
        let mut registry = RuntimeAuiDocumentRegistry::empty("subscriptions");
        let document = AuiDocument::new(
            "ui",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![AuiNode::new(
                "root",
                AuiNodeKind::Panel,
                AuiRect::stretch_full(),
            )],
        );
        registry.documents_by_id.insert("ui".into(), document);
        let mut session = AuiSession::mount(&registry).unwrap();
        let root = session.resolve("ui", "root").unwrap();

        session.begin();
        session
            .subscribe(
                root,
                UiEventFilter::Lifecycle,
                UiSubscriptionScope::Instance,
                UiCallbackId::new(7).unwrap(),
            )
            .unwrap();
        session.finish(false);
        assert_eq!(session.subscription_count_for_test(), 0);

        session.begin();
        let token = session
            .subscribe(
                root,
                UiEventFilter::Lifecycle,
                UiSubscriptionScope::Instance,
                UiCallbackId::new(7).unwrap(),
            )
            .unwrap();
        session.finish(true);
        assert_eq!(session.subscription_count_for_test(), 1);
        let mount = session.next_dispatch_call().unwrap().unwrap();
        assert_eq!(mount.callback_id, UiCallbackId(7));
        assert_eq!(mount.notification.kind, UiNotificationKind::Mount);
        let enable = session.next_dispatch_call().unwrap().unwrap();
        assert_eq!(enable.notification.kind, UiNotificationKind::Enable);
        assert!(session.next_dispatch_call().unwrap().is_none());

        session.begin();
        assert!(session.cancel(token).unwrap());
        session.finish(false);
        assert_eq!(session.subscription_count_for_test(), 1);
        session.begin();
        assert!(session.cancel(token).unwrap());
        session.finish(true);
        assert_eq!(session.subscription_count_for_test(), 0);
        session.begin();
        assert!(!session.cancel(token).unwrap());
        session.finish(true);
        session.shutdown();
        assert_eq!(session.subscription_count_for_test(), 0);
        assert!(session.resolve("ui", "root").is_err());
    }

    fn structural_registry() -> RuntimeAuiDocumentRegistry {
        let mut registry = RuntimeAuiDocumentRegistry::empty("structure");
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.children = vec!["left".into(), "right".into(), "grid".into()];
        let left = AuiNode::new(
            "left",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0., 0., 200., 300.),
        )
        .with_parent("root");
        let right = AuiNode::new(
            "right",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(300., 0., 200., 300.),
        )
        .with_parent("root");
        let mut grid = AuiNode::new(
            "grid",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(500., 0., 200., 300.),
        )
        .with_parent("root");
        grid.layout = Some(LayoutSpec {
            mode: LayoutMode::Grid,
            ..Default::default()
        });
        let mut base = AuiDocument::new(
            "base",
            vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
            vec![root, left, right, grid],
        );
        base.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
        registry.documents_by_id.insert("base".into(), base);

        let mut card = AuiNode::new(
            "card",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(10., 20., 80., 40.),
        );
        card.children.push("label".into());
        let label = AuiNode::new(
            "label",
            AuiNodeKind::Text,
            AuiRect::fixed_position(5., 5., 60., 20.),
        )
        .with_parent("card")
        .with_text("人");
        registry.documents_by_id.insert(
            "card_template".into(),
            AuiDocument::new(
                "card_template",
                vec![AuiCanvas::screen_overlay("template", 80., 40., "card")],
                vec![card, label],
            ),
        );
        registry
    }

    fn animation_play(
        target: UiNodeHandle,
        completion: project_runtime_sdk::aui_animation::CompletionPolicy,
    ) -> project_runtime_sdk::aui_animation::AnimationRequest {
        use project_runtime_sdk::aui_animation::*;
        AnimationRequest::Play {
            request: AnimationPlay {
                owner: target,
                scope: UiSubscriptionScope::Active,
                clock: AnimationClock::Ui,
                conflict: TrackConflictPolicy::Reject,
                callback_id: Some(UiCallbackId(123)),
                animation: Animation2D {
                    version: ANIMATION_2D_VERSION,
                    root: AnimationStep::Tween {
                        track: TweenTrack {
                            target,
                            property: TweenProperty::Opacity,
                            from: TweenValue::Scalar(0.),
                            to: TweenValue::Scalar(0.7),
                            delay_us: 0,
                            duration_us: 100,
                            easing: Easing::Linear,
                        },
                    },
                    repeat: Repeat::Once,
                    direction: PlaybackDirection::Forward,
                    completion,
                },
            },
        }
    }

    #[test]
    fn aui_animation_callback_rollback_never_activates_reserved_handle_or_clock_changes() {
        use project_runtime_sdk::aui_animation::*;
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let target = session.resolve("base", "left").unwrap();
        session.begin();
        let AnimationReply::Handle(rolled_back) = session
            .animation_request(animation_play(target, CompletionPolicy::Hold))
            .unwrap()
        else {
            panic!()
        };
        session
            .animation_request(AnimationRequest::SetGameClock {
                paused: true,
                speed: 2.,
            })
            .unwrap();
        session.finish(false);
        session.advance_animations(1, 20).unwrap();
        assert!(session
            .document("base")
            .unwrap()
            .presentation_property_at(target.slot, UiProperty::Opacity)
            .is_none());
        assert!(session.next_dispatch_call().unwrap().is_none());
        session.begin();
        let AnimationReply::Handle(live) = session
            .animation_request(animation_play(target, CompletionPolicy::Hold))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(live.slot, rolled_back.slot);
        assert_ne!(live.generation, rolled_back.generation);
        assert_eq!(
            session
                .animation_request(AnimationRequest::SetPaused {
                    handle: rolled_back,
                    paused: true
                })
                .unwrap_err()
                .kind,
            UiErrorKind::InvalidHandle
        );
        session.finish(true);
        session.advance_animations(2, 50).unwrap();
        assert_eq!(
            session
                .document("base")
                .unwrap()
                .presentation_property_at(target.slot, UiProperty::Opacity)
                .unwrap()
                .1,
            UiValue::Number(0.35)
        );
        session.begin();
        session
            .animation_request(AnimationRequest::Cancel { handle: live })
            .unwrap();
        session.finish(false);
        session.advance_animations(3, 50).unwrap();
        let event = session.next_dispatch_call().unwrap().unwrap();
        assert_eq!(event.callback_id, UiCallbackId(123));
        assert_eq!(
            event.notification.kind,
            UiNotificationKind::Animation {
                handle: live,
                outcome: AnimationOutcome::Completed
            }
        );
        assert!(session.next_dispatch_call().unwrap().is_none());
        session.shutdown();
        assert!(session.next_dispatch_call().unwrap().is_none());
        assert!(session
            .animation_request(AnimationRequest::Cancel { handle: live })
            .is_err());
    }

    #[test]
    fn aui_animation_commit_captures_accepted_same_callback_base_revision() {
        use project_runtime_sdk::aui_animation::*;
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let target = session.resolve("base", "left").unwrap();
        session.begin();
        session
            .stage(
                UiUpdateBatch::new(vec![UiPropertyUpdate {
                    handle: target,
                    property: UiProperty::Opacity,
                    value: UiValue::Number(0.3),
                    notify: false,
                }])
                .unwrap(),
            )
            .unwrap();
        session
            .animation_request(animation_play(target, CompletionPolicy::CommitFinal))
            .unwrap();
        session.finish(true);
        assert_eq!(
            session.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.3)
        );
        session.advance_animations(1, 100).unwrap();
        assert_eq!(
            session.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.7)
        );
        assert!(matches!(
            session
                .next_dispatch_call()
                .unwrap()
                .unwrap()
                .notification
                .kind,
            UiNotificationKind::Animation {
                outcome: AnimationOutcome::Completed,
                ..
            }
        ));
    }

    #[test]
    fn aui_animation_binding_base_remains_owned_and_release_uses_latest_published_base() {
        use project_runtime_sdk::aui_animation::*;
        let mut registry = structural_registry();
        registry.documents_by_id.get_mut("base").unwrap().nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "opacity",
                AuiBindingTarget::Opacity,
                "game.opacity",
                None,
            ));
        let mut session = AuiSession::mount(&registry).unwrap();
        let target = session.resolve("base", "left").unwrap();
        session.begin();
        let AnimationReply::Handle(h) = session
            .animation_request(animation_play(target, CompletionPolicy::Hold))
            .unwrap()
        else {
            panic!()
        };
        session.finish(true);
        session.advance_animations(1, 50).unwrap();
        let mut document = session.document("base").unwrap().source().clone();
        document.nodes[1].binding_refs.clear();
        document.nodes[1].presentation =
            Some(project_runtime_sdk::aui_presentation::NodePresentation {
                opacity: 0.4,
                ..Default::default()
            });
        let output = AuiRuntimePresenter::present_package_smoke(&document, 1);
        assert!(output.layout.diagnostics.is_empty());
        session.publish_geometry(&output);
        assert_eq!(
            session.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.4)
        );
        assert_eq!(
            session
                .document("base")
                .unwrap()
                .animation_stamp(target, UiProperty::Opacity)
                .unwrap()
                .0,
            UiPropertyOwner::Binding
        );
        session.begin();
        session
            .animation_request(AnimationRequest::Cancel { handle: h })
            .unwrap();
        session.finish(true);
        assert!(session
            .document("base")
            .unwrap()
            .presentation_property_at(target.slot, UiProperty::Opacity)
            .is_none());
        session.begin();
        session
            .animation_request(animation_play(target, CompletionPolicy::CommitFinal))
            .unwrap();
        session.finish(true);
        session.advance_animations(2, 100).unwrap();
        // Drain earlier cancel event, then check commit result without stealing the binding.
        let _ = session.next_dispatch_call().unwrap();
        assert!(matches!(
            session
                .next_dispatch_call()
                .unwrap()
                .unwrap()
                .notification
                .kind,
            UiNotificationKind::Animation {
                outcome: AnimationOutcome::CommitConflict,
                ..
            }
        ));
        assert_eq!(
            session.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.4)
        );
    }

    #[test]
    fn aui_animation_dynamic_target_deletion_and_generation_reuse_cancel_old_playback() {
        use project_runtime_sdk::aui_animation::*;
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        let old = instantiate(&mut session, parent, 1)[0].root;
        session.begin();
        session
            .animation_request(animation_play(old, CompletionPolicy::Hold))
            .unwrap();
        session.finish(true);
        session.advance_animations(1, 50).unwrap();
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Delete { target: old },
            ]))
            .unwrap();
        session.finish(true);
        let next = instantiate(&mut session, parent, 1)[0].root;
        assert_eq!(next.slot, old.slot);
        assert_ne!(next.generation, old.generation);
        assert_eq!(
            session.read(next, UiProperty::Opacity).unwrap(),
            UiValue::Number(1.)
        );
        session.advance_animations(2, 50).unwrap();
        let mut disposed_events = 0;
        while let Some(event) = session.next_dispatch_call().unwrap() {
            if matches!(
                event.notification.kind,
                UiNotificationKind::Animation {
                    outcome: AnimationOutcome::TargetDisposed,
                    ..
                }
            ) {
                disposed_events += 1;
            }
        }
        assert_eq!(disposed_events, 1);
        assert_eq!(
            session.read(next, UiProperty::Opacity).unwrap(),
            UiValue::Number(1.)
        );
    }

    fn instantiate(
        session: &mut AuiSession,
        parent: UiNodeHandle,
        count: usize,
    ) -> Vec<UiTemplateInstance> {
        session.begin();
        let result = session
            .structure(UiStructureTransaction::new(
                (0..count)
                    .map(|_| UiStructureOperation::Instantiate {
                        template: "card_template".into(),
                        parent,
                        sibling_index: None,
                    })
                    .collect(),
            ))
            .unwrap();
        session.finish(true);
        result.instantiated
    }

    #[test]
    fn gate_c_structure_transaction_preserves_independent_canvas_layout_and_failed_draft() {
        let mut registry = structural_registry();
        let base = registry.documents_by_id.get_mut("base").unwrap();
        let independent = AuiNode::new("independent", AuiNodeKind::Panel, AuiRect::stretch_full());
        base.nodes.push(independent);
        base.canvases.push(AuiCanvas::screen_overlay(
            "second",
            300.,
            200.,
            "independent",
        ));
        let mut session = AuiSession::mount(&registry).unwrap();
        let mut measure = Measure;
        for canvas in ["main", "second"] {
            session
                .layout_document("base", canvas, &mut measure)
                .unwrap();
        }
        let parent = session.resolve("base", "left").unwrap();
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Instantiate {
                    template: "card_template".into(),
                    parent,
                    sibling_index: None,
                },
            ]))
            .unwrap();
        session.finish(false);
        for canvas in ["main", "second"] {
            session
                .layout_document("base", canvas, &mut measure)
                .unwrap();
            assert!(
                session
                    .document("base")
                    .unwrap()
                    .layout_stats(canvas)
                    .unwrap()
                    .clean_snapshot_reused
            );
        }
        let instance = instantiate(&mut session, parent, 1).remove(0);
        for canvas in ["main", "second"] {
            let actual = session
                .layout_document("base", canvas, &mut measure)
                .unwrap()
                .clone();
            let source = session.document("base").unwrap().source();
            let expected = AuiLayoutSolver::new()
                .solve(source, canvas, 1, &mut measure)
                .unwrap()
                .clone();
            assert_eq!(actual.nodes, expected.nodes);
        }
        assert!(
            session
                .document("base")
                .unwrap()
                .layout_stats("main")
                .unwrap()
                .source_nodes_loaded
                > 0
        );
        assert!(
            session
                .document("base")
                .unwrap()
                .layout_stats("second")
                .unwrap()
                .clean_snapshot_reused
        );
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Delete {
                    target: instance.root,
                },
            ]))
            .unwrap();
        session.finish(true);
        session
            .layout_document("base", "second", &mut measure)
            .unwrap();
        assert!(
            session
                .document("base")
                .unwrap()
                .layout_stats("second")
                .unwrap()
                .clean_snapshot_reused
        );
        assert_eq!(
            session.node_type(instance.root).unwrap_err().kind,
            UiErrorKind::InvalidHandle
        );
    }

    #[test]
    fn template_instances_are_isolated_and_deleted_slots_never_revive_old_handles() {
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        let instances = instantiate(&mut session, parent, 2);
        assert_ne!(
            instances[0].template_instance_id,
            instances[1].template_instance_id
        );
        let first_label = instances[0].node("label").unwrap().handle();
        let second_label = instances[1].node("label").unwrap().handle();
        assert_ne!(first_label, second_label);
        assert_eq!(
            session.read(first_label, UiProperty::Text).unwrap(),
            UiValue::Text("人".into())
        );
        assert!(session
            .document("base")
            .unwrap()
            .is_effectively_active(instances[0].root)
            .unwrap());
        session.begin();
        session
            .stage(
                UiUpdateBatch::new(vec![UiPropertyUpdate {
                    handle: parent,
                    property: UiProperty::Active,
                    value: UiValue::Bool(false),
                    notify: true,
                }])
                .unwrap(),
            )
            .unwrap();
        session.finish(true);
        assert!(!session
            .document("base")
            .unwrap()
            .is_effectively_active(instances[0].root)
            .unwrap());
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::SetSiblingIndex {
                    target: instances[1].root,
                    sibling_index: 0,
                },
            ]))
            .unwrap();
        session.finish(true);
        let document = session.document("base").unwrap();
        let first_child = document.source().nodes[parent.slot as usize].children[0].clone();
        assert_eq!(
            first_child,
            document.source().nodes[instances[1].root.slot as usize].node_id
        );

        let old_root = instances[0].root;
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Delete { target: old_root },
            ]))
            .unwrap();
        session.finish(true);
        assert_eq!(
            session.node_type(old_root).unwrap_err().kind,
            UiErrorKind::InvalidHandle
        );
        assert_eq!(session.node_type(second_label).unwrap(), UiNodeType::Text);

        let replacement = instantiate(&mut session, parent, 1).remove(0);
        assert_eq!(replacement.root.slot, old_root.slot);
        assert_ne!(replacement.root.generation, old_root.generation);
        assert_eq!(
            session.node_type(old_root).unwrap_err().kind,
            UiErrorKind::InvalidHandle
        );
        session.begin();
        let other_root = session.resolve("card_template", "card").unwrap();
        let error = session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: replacement.root,
                    new_parent: other_root,
                    sibling_index: None,
                    mode: UiReparentMode::KeepLocal,
                },
            ]))
            .unwrap_err();
        assert_eq!(error.kind, UiErrorKind::IncompatibleRoot);
        session.finish(false);
    }

    #[test]
    fn custom_outputs_remain_isolated_across_two_template_instances_and_shutdown() {
        use project_runtime_sdk::aui_extension::{UiExtensionMeasureOutput, UiExtensionOutput};

        let mut registry = structural_registry();
        let mut radial = AuiNode::new(
            "radial",
            AuiNodeKind::Custom,
            AuiRect::fixed_position(0., 0., 120., 120.),
        );
        radial.custom = Some(crate::aui_extension::AuiCustomNodeSpec {
            type_id: "sample.radial".into(),
            schema_version: 1,
            properties: BTreeMap::new(),
        });
        registry.documents_by_id.insert(
            "radial_template".into(),
            AuiDocument::new(
                "radial_template",
                vec![AuiCanvas::screen_overlay("template", 120., 120., "radial")],
                vec![radial],
            ),
        );

        let mut session = AuiSession::mount(&registry).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        session.begin();
        let instances = session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Instantiate {
                    template: "radial_template".into(),
                    parent,
                    sibling_index: None,
                },
                UiStructureOperation::Instantiate {
                    template: "radial_template".into(),
                    parent,
                    sibling_index: None,
                },
            ]))
            .unwrap()
            .instantiated;
        session.finish(true);
        assert_eq!(instances.len(), 2);
        assert_ne!(instances[0].root, instances[1].root);

        for (instance, width) in instances.iter().zip([40.0, 80.0]) {
            session
                .install_custom_output(
                    instance.root,
                    UiExtensionOutput::Measure {
                        output: UiExtensionMeasureOutput {
                            size: [width, 24.0],
                            first_baseline: None,
                        },
                    },
                )
                .unwrap();
        }
        let base = session.document("base").unwrap();
        for (instance, width) in instances.iter().zip([40.0, 80.0]) {
            let output = base.source().nodes[instance.root.slot as usize]
                .custom_runtime
                .as_ref()
                .unwrap();
            assert_eq!(output.measure.as_ref().unwrap().size, [width, 24.0]);
        }

        session.shutdown();
        assert!(session.custom_work().is_empty());
        assert!(session.resolve("base", "root").is_err());
    }

    #[test]
    fn failed_structure_transaction_rolls_back_prior_property_and_tree_changes() {
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        let instance = instantiate(&mut session, parent, 1).remove(0);
        let label = instance.node("label").unwrap().handle();
        session.begin();
        session
            .stage(
                UiUpdateBatch::new(vec![UiPropertyUpdate {
                    handle: label,
                    property: UiProperty::Text,
                    value: UiValue::Text("骑兵".into()),
                    notify: true,
                }])
                .unwrap(),
            )
            .unwrap();
        let error = session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::SetSiblingIndex {
                    target: instance.root,
                    sibling_index: 0,
                },
                UiStructureOperation::Reparent {
                    target: instance.root,
                    new_parent: label,
                    sibling_index: None,
                    mode: UiReparentMode::KeepLocal,
                },
            ]))
            .unwrap_err();
        assert_eq!(error.kind, UiErrorKind::HierarchyCycle);
        session.finish(false);
        assert_eq!(
            session.read(label, UiProperty::Text).unwrap(),
            UiValue::Text("人".into())
        );
    }

    #[test]
    fn reparent_modes_and_coordinate_queries_use_committed_geometry() {
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let left = session.resolve("base", "left").unwrap();
        let right = session.resolve("base", "right").unwrap();
        let grid = session.resolve("base", "grid").unwrap();
        let instances = instantiate(&mut session, left, 2);
        let keep_screen = instances[0].root;
        let keep_local = instances[1].root;
        let mut measure = Measure;
        session
            .layout_document("base", "main", &mut measure)
            .unwrap();
        let before = session.local_to_screen(keep_screen, [3., 4.]).unwrap();
        assert_eq!(
            session.screen_to_local(keep_screen, before).unwrap(),
            [3., 4.]
        );

        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: keep_screen,
                    new_parent: right,
                    sibling_index: Some(0),
                    mode: UiReparentMode::KeepScreen,
                },
            ]))
            .unwrap();
        session.finish(true);
        session
            .layout_document("base", "main", &mut measure)
            .unwrap();
        let after = session.local_to_screen(keep_screen, [3., 4.]).unwrap();
        assert!((before[0] - after[0]).abs() < 0.001);
        assert!((before[1] - after[1]).abs() < 0.001);

        session
            .layout_document("base", "main", &mut measure)
            .unwrap();
        let local_before = session.local_to_screen(keep_local, [0., 0.]).unwrap();
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: keep_local,
                    new_parent: right,
                    sibling_index: None,
                    mode: UiReparentMode::KeepLocal,
                },
            ]))
            .unwrap();
        session.finish(true);
        session
            .layout_document("base", "main", &mut measure)
            .unwrap();
        let local_after = session.local_to_screen(keep_local, [0., 0.]).unwrap();
        assert!((local_after[0] - local_before[0] - 300.).abs() < 0.001);

        session.begin();
        let error = session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: keep_local,
                    new_parent: grid,
                    sibling_index: None,
                    mode: UiReparentMode::KeepScreen,
                },
            ]))
            .unwrap_err();
        assert_eq!(error.kind, UiErrorKind::ControlledLayout);
        session.finish(false);
    }

    #[test]
    fn affine_keep_screen_reparent_preserves_pivot_origin_and_rejects_singular_parent() {
        use project_runtime_sdk::aui_wire::UiWriteBuffer;
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let left = session.resolve("base", "left").unwrap();
        let right = session.resolve("base", "right").unwrap();
        let child = instantiate(&mut session, left, 1)[0].root;
        session
            .layout_document("base", "main", &mut Measure)
            .unwrap();
        session.begin();
        let mut writes = UiWriteBuffer::new(&session);
        UiNodeRef::new(right)
            .set_rotation(&mut writes, 0.7)
            .unwrap();
        UiNodeRef::new(right)
            .set_scale(&mut writes, [-2., 0.5])
            .unwrap();
        UiNodeRef::new(child)
            .set_rotation(&mut writes, 0.2)
            .unwrap();
        UiNodeRef::new(child)
            .set_pivot(&mut writes, [0.3, 0.8])
            .unwrap();
        session.stage(writes.finish().unwrap()).unwrap();
        session.finish(true);
        let before = session.local_to_screen(child, [0., 0.]).unwrap();
        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: child,
                    new_parent: right,
                    sibling_index: None,
                    mode: UiReparentMode::KeepScreen,
                },
            ]))
            .unwrap();
        session.finish(true);
        session
            .layout_document("base", "main", &mut Measure)
            .unwrap();
        let after = session.local_to_screen(child, [0., 0.]).unwrap();
        assert!(
            (before[0] - after[0]).abs() < 1e-3 && (before[1] - after[1]).abs() < 1e-3,
            "{before:?} {after:?}"
        );
        session.begin();
        let mut writes = UiWriteBuffer::new(&session);
        UiNodeRef::new(left)
            .set_scale(&mut writes, [0., 1.])
            .unwrap();
        session.stage(writes.finish().unwrap()).unwrap();
        session.finish(true);
        session.begin();
        let error = session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Reparent {
                    target: child,
                    new_parent: left,
                    sibling_index: None,
                    mode: UiReparentMode::KeepScreen,
                },
            ]))
            .unwrap_err();
        assert_eq!(error.kind, UiErrorKind::NonInvertibleTransform);
        session.finish(false);
    }

    #[test]
    fn published_affine_geometry_drives_public_queries_dragging_and_singular_errors() {
        use crate::aui::*;
        use project_runtime_sdk::aui_wire::UiWriteBuffer;
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        let parent_ref = UiNodeRef::new(parent);
        session.begin();
        let mut writes = UiWriteBuffer::new(&session);
        parent_ref
            .set_rotation(&mut writes, std::f32::consts::FRAC_PI_2)
            .unwrap();
        parent_ref.set_scale(&mut writes, [-2., 1.]).unwrap();
        parent_ref.set_pivot(&mut writes, [0., 0.]).unwrap();
        let batch = writes.finish().unwrap();
        session.stage(batch).unwrap();
        session.finish(true);
        let document = session.document("base").unwrap().source().clone();
        let mut runtime = AuiC2Runtime::default();
        runtime.observe_retained(session.document("base"));
        let out = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
            &document,
            ProjectUiStateSnapshotOutput::new(
                "test",
                AuiSnapshotSource::TestSnapshot,
                ProjectUiStateSnapshot::new(1),
            ),
            &crate::runtime_package::RuntimeAuiFontAtlasRegistry::empty("test"),
            &crate::font_bundle::RuntimeFontBundleRegistry::default(),
            None,
            &mut runtime,
        );
        assert!(session.publish_geometry(&out) > 0);
        assert_eq!(
            session.publish_geometry(&out),
            0,
            "clean publication must be O(1)"
        );
        let point = parent_ref.local_to_screen(&session, [10., 20.]).unwrap();
        let local = parent_ref.screen_to_local(&session, point).unwrap();
        assert!((local[0] - 10.).abs() < 1e-4 && (local[1] - 20.).abs() < 1e-4);
        let shifted = parent_ref
            .screen_to_local(&session, [point[0], point[1] + 20.])
            .unwrap();
        assert!(
            (shifted[0]).abs() < 1e-4 && (shifted[1] - 20.).abs() < 1e-4,
            "drag must use parent inverse: {shifted:?}"
        );
        let query: UiQuery = serde_json::from_slice(
            &serde_json::to_vec(&UiQuery::ScreenToLocal {
                handle: parent,
                point,
            })
            .unwrap(),
        )
        .unwrap();
        match session.query(query).unwrap() {
            UiQueryValue::Point(value) => assert_eq!(value, local),
            other => panic!("{other:?}"),
        };
        session.begin();
        let mut writes = UiWriteBuffer::new(&session);
        parent_ref.set_scale(&mut writes, [0., 1.]).unwrap();
        let batch = writes.finish().unwrap();
        session.stage(batch).unwrap();
        session.finish(true);
        assert_eq!(
            parent_ref
                .screen_to_local(&session, point)
                .unwrap_err()
                .kind,
            UiErrorKind::NonInvertibleTransform
        );
    }

    #[test]
    fn delete_dispatches_lifecycle_then_releases_target_subscription() {
        let mut session = AuiSession::mount(&structural_registry()).unwrap();
        let parent = session.resolve("base", "left").unwrap();
        let target = instantiate(&mut session, parent, 1).remove(0).root;
        session.begin();
        session
            .subscribe(
                target,
                UiEventFilter::Lifecycle,
                UiSubscriptionScope::Instance,
                UiCallbackId::new(99).unwrap(),
            )
            .unwrap();
        session.finish(true);
        assert!(session.next_dispatch_call().unwrap().is_some());
        assert!(session.next_dispatch_call().unwrap().is_some());
        assert!(session.next_dispatch_call().unwrap().is_none());

        session.begin();
        session
            .structure(UiStructureTransaction::new(vec![
                UiStructureOperation::Delete { target },
            ]))
            .unwrap();
        session.finish(true);
        let disable = session.next_dispatch_call().unwrap().unwrap();
        let dispose = session.next_dispatch_call().unwrap().unwrap();
        assert_eq!(disable.notification.kind, UiNotificationKind::Disable);
        assert_eq!(dispose.notification.kind, UiNotificationKind::Dispose);
        assert!(session.next_dispatch_call().unwrap().is_none());
        assert_eq!(session.subscription_count_for_test(), 0);
    }
}
