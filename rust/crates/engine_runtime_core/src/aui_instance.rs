//! Retained AUI property owner. Mount reads authoring once; edits only visit touched properties.
//! Layout and Player presentation consume committed values without modifying source authoring.

use crate::aui::{
    AuiBindingTarget, AuiComputedRect, AuiDocument, AuiNode, AuiNodeKind, AuiRect,
    AuiValidationItem,
};
use crate::aui_layout_runtime::{
    AuiIntrinsicMeasure, AuiLayoutSnapshot, AuiLayoutSolver, AuiLayoutSource, AuiLayoutStats,
};
use crate::aui_layout_schema::{self, LayoutVisibility};
use project_runtime_sdk::aui::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::ops::Bound::{Excluded, Included};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub const MAX_AUI_EDIT_PROPERTIES: usize = project_runtime_sdk::aui_wire::MAX_UI_UPDATE_PROPERTIES;
pub const MAX_AUI_EDIT_TEXT_BYTES: usize = project_runtime_sdk::aui_wire::MAX_UI_UPDATE_TEXT_BYTES;
pub const MAX_AUI_SUBSCRIPTIONS: usize = 4096;
const MAX_PRESENTATION_CHANGES: usize = 8192;
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
static NEXT_DYNAMIC_GENERATION: AtomicU32 = AtomicU32::new(2);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AuiInvalidation {
    pub style: bool,
    pub measure: bool,
    pub layout: bool,
    pub paint: bool,
    pub transform: bool,
    pub hit: bool,
}

impl AuiInvalidation {
    fn merge(&mut self, other: Self) {
        self.style |= other.style;
        self.measure |= other.measure;
        self.layout |= other.layout;
        self.paint |= other.paint;
        self.transform |= other.transform;
        self.hit |= other.hit;
    }

    fn is_empty(self) -> bool {
        !(self.style || self.measure || self.layout || self.paint || self.transform || self.hit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuiInvalidationEntry {
    pub handle: UiNodeHandle,
    pub invalidation: AuiInvalidation,
}

pub type AuiChangeOrigin = UiChangeOrigin;
pub type AuiNotificationKind = UiNotificationKind;
pub type AuiNotification = UiNotification;
pub type AuiSubscriptionScope = UiSubscriptionScope;
pub type AuiEventFilter = UiEventFilter;
pub type AuiSubscriptionToken = UiSubscriptionToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AuiPresentationChange {
    Property(UiProperty),
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuiDispatchListener {
    pub token: AuiSubscriptionToken,
    pub callback_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuiDispatchError {
    Reentrant,
    NotDispatching,
}

#[derive(Clone)]
struct AuiSubscription {
    target: UiNodeHandle,
    filter: AuiEventFilter,
    scope: AuiSubscriptionScope,
    callback_id: u64,
}

#[derive(Clone)]
struct QueuedNotification {
    notification: AuiNotification,
    only_token: Option<u64>,
    active_at_enqueue: bool,
}

#[derive(Clone)]
struct AuiDispatchState {
    queued: QueuedNotification,
    max_token: u64,
    cursor: u64,
}

#[derive(Clone)]
struct NodeState {
    source_node: AuiNode,
    alive: bool,
    kind: UiNodeType,
    generation: u32,
    text: String,
    value: f32,
    translation: [f32; 2],
    presentation: project_runtime_sdk::aui_presentation::NodePresentation,
    resource: Option<UiResourceRef>,
    local_style: UiLocalStyle,
    interactable: bool,
    visibility: UiVisibility,
    active: bool,
    active_in_hierarchy: bool,
    parent: Option<u32>,
    children: Vec<u32>,
    canvas_root: u32,
    template_instance_id: Option<u64>,
    binding_owned: [bool; 13],
    dirty: AuiInvalidation,
    queued: bool,
}

/// One mounted document instance. Shared authoring is immutable and never used for edit diffing.
pub struct AuiInstance {
    animation_values: BTreeMap<(u32, UiProperty), UiValue>,
    base_revisions: BTreeMap<(u32, UiProperty), u64>,
    source: Arc<AuiDocument>,
    identity: u64,
    nodes: Vec<NodeState>,
    by_id: HashMap<String, u32>,
    dirty: Vec<u32>,
    disposed: bool,
    layouts: HashMap<String, AuiLayoutSolver>,
    layout_source: Option<Arc<crate::aui_layout_runtime::AuiLayoutSource>>,
    presentation_revision: u64,
    content_revision: u64,
    structure_revision: u64,
    // Explicitly set property slots, not a history or a second copy of the text values.
    // Binding-owned fields never enter this set.
    presentation_properties: BTreeSet<(u32, UiProperty)>,
    presentation_changes: VecDeque<(u64, u32, AuiPresentationChange)>,
    presentation_change_floor: u64,
    subscriptions: BTreeMap<u64, AuiSubscription>,
    next_subscription_id: u64,
    notifications: VecDeque<QueuedNotification>,
    dispatch: Option<AuiDispatchState>,
    next_notification_sequence: u64,
    canvas_roots: BTreeSet<u32>,
    next_template_instance_id: u64,
    committed_geometry: HashMap<u32, AuiComputedRect>,
    published_layout: Option<Arc<crate::aui::AuiLayoutResult>>,
    published_revision: u64,
    published_matrices: HashMap<u32, project_runtime_sdk::aui_presentation::Affine2D>,
    pending_keep_screen: BTreeMap<u32, [f32; 2]>,
    retired_handles: BTreeSet<UiNodeHandle>,
    custom_dirty: BTreeMap<u32, crate::aui_extension::AuiCustomDirty>,
}

impl AuiInstance {
    pub(crate) fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        owner: &str,
        role: &str,
    ) {
        fn value(
            value: &UiValue,
            sample: &mut crate::resource_memory::ResourceMemorySample,
            owner: &str,
            role: &str,
        ) {
            match value {
                UiValue::Text(text) => sample.cpu_string(text, owner, &[role]),
                UiValue::Resource(Some(resource)) => {
                    sample.cpu_string(&resource.asset_id, owner, &[role]);
                    sample.cpu_string(&resource.guid, owner, &[role]);
                }
                _ => {}
            }
        }
        fn notification(
            notification: &UiNotification,
            sample: &mut crate::resource_memory::ResourceMemorySample,
        ) {
            if let UiNotificationKind::Changed {
                old_value,
                new_value,
                ..
            } = &notification.kind
            {
                value(old_value, sample, "queues", "queuePayload");
                value(new_value, sample, "queues", "queuePayload");
            }
        }
        sample.cpu_vec(&self.nodes, owner, &[role]);
        sample.cpu_vec(&self.dirty, owner, &[role]);
        for node in &self.nodes {
            sample.cpu_string(&node.text, owner, &[role]);
            sample.cpu_vec(&node.children, owner, &[role]);
            if let Some(resource) = &node.resource {
                sample.cpu_string(&resource.asset_id, owner, &[role]);
                sample.cpu_string(&resource.guid, owner, &[role]);
            }
        }
        for key in self.by_id.keys().chain(self.layouts.keys()) {
            sample.cpu_string(key, owner, &[role]);
        }
        for current in self.animation_values.values() {
            value(current, sample, owner, role);
        }
        sample.cpu_deque(&self.presentation_changes, "queues", &["queuePayload"]);
        sample.cpu_deque(&self.notifications, "queues", &["queuePayload"]);
        for queued in &self.notifications {
            notification(&queued.notification, sample);
        }
        if let Some(dispatch) = &self.dispatch {
            notification(&dispatch.queued.notification, sample);
        }
        sample.partial(
            &format!("cpu.owner.{owner}"),
            "sdk.ui.instance.nodes+dirty+children+currentTextResource+lookupKeys+animationValues.actualBacking;sourceNodeAuthoring+layout+customBackingNotIntegrated;stdMapNodesOpaque",
        );
        sample.partial(
            "cpu.owner.queues",
            "sdk.ui.instance.notification+dispatchPayload+presentationJournal.actualBacking",
        );
    }

    pub(crate) fn animation_stamp(
        &self,
        handle: UiNodeHandle,
        property: UiProperty,
    ) -> Result<(UiPropertyOwner, u64), UiError> {
        let node = self.state(handle)?;
        Ok((
            if node.binding_owned[property_index(property)] {
                UiPropertyOwner::Binding
            } else {
                UiPropertyOwner::Controller
            },
            self.base_revisions
                .get(&(handle.slot, property))
                .copied()
                .unwrap_or(0),
        ))
    }

    pub(crate) fn set_animation_value(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: Option<UiValue>,
    ) -> Result<(), UiError> {
        self.state(handle)?;
        let key = (handle.slot, property);
        if self.animation_values.get(&key) == value.as_ref() {
            return Ok(());
        }
        if let Some(value) = value {
            self.animation_values.insert(key, value);
        } else {
            self.animation_values.remove(&key);
        }
        self.presentation_revision = self
            .presentation_revision
            .checked_add(1)
            .expect("UI revision exhausted");
        self.record_presentation_change(handle.slot, property);
        Ok(())
    }

    fn presentation_value(&self, slot: u32, property: UiProperty) -> UiValue {
        self.animation_values
            .get(&(slot, property))
            .cloned()
            .unwrap_or_else(|| node_value(&self.nodes[slot as usize], property))
    }

    /// Called at successful presentation publication, never from a half-accepted callback.
    /// Clean frames are O(1); C2 journal entries restrict dirty updates to their dependency closure.
    pub(crate) fn publish_geometry(
        &mut self,
        document: &AuiDocument,
        layout: &Arc<crate::aui::AuiLayoutResult>,
    ) -> usize {
        if self
            .published_layout
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, layout))
        {
            return 0;
        }
        let changes = self
            .published_layout
            .as_ref()
            .and_then(|old| layout.changed_slots_since(old));
        let slots: Box<dyn Iterator<Item = usize>> = match changes {
            Some(slots) => Box::new(slots.into_iter()),
            None => {
                self.published_matrices.clear();
                Box::new(0..layout.computed_nodes.len())
            }
        };
        let mut count = 0;
        for index in slots {
            let computed = &layout.computed_nodes[index];
            let Some(&slot) = self.by_id.get(&computed.node_id) else {
                continue;
            };
            let node = &mut self.nodes[slot as usize];
            // Authoring order is stable in C2. Dynamic structure keeps by_id authoritative.
            if let Some(resolved) = document
                .nodes
                .get(slot as usize)
                .filter(|n| n.node_id == computed.node_id)
                .or_else(|| {
                    document
                        .nodes
                        .iter()
                        .find(|n| n.node_id == computed.node_id)
                })
            {
                let bound = resolved.presentation.unwrap_or_default();
                for property in [
                    UiProperty::Translation,
                    UiProperty::Rotation,
                    UiProperty::Scale,
                    UiProperty::Pivot,
                    UiProperty::Opacity,
                    UiProperty::Tint,
                ] {
                    if node.binding_owned[property_index(property)] {
                        let old = node_value(node, property);
                        match property {
                            UiProperty::Translation => {
                                node.translation = bound.transform.translation
                            }
                            UiProperty::Rotation => {
                                node.presentation.transform.rotation = bound.transform.rotation
                            }
                            UiProperty::Scale => {
                                node.presentation.transform.scale = bound.transform.scale
                            }
                            UiProperty::Pivot => {
                                node.presentation.transform.pivot = bound.transform.pivot
                            }
                            UiProperty::Opacity => node.presentation.opacity = bound.opacity,
                            UiProperty::Tint => node.presentation.tint = bound.tint,
                            _ => unreachable!(),
                        }
                        if old != node_value(node, property) {
                            *self.base_revisions.entry((slot, property)).or_default() += 1;
                        }
                    }
                }
            }
            let translation = self.accumulated_translation(slot, true);
            let mut rect = computed.rect;
            rect.x -= translation[0];
            rect.y -= translation[1];
            self.committed_geometry.insert(slot, rect);
            if let Some(geometry) = computed.presentation {
                if let Ok(matrix) =
                    project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(
                        geometry.local_to_canvas,
                    )
                {
                    self.published_matrices.insert(slot, matrix);
                }
            } else if let Ok(matrix) = project_runtime_sdk::aui_presentation::Transform2D::default()
                .local_matrix(
                    [computed.rect.x, computed.rect.y],
                    [rect.width, rect.height],
                )
            {
                self.published_matrices.insert(slot, matrix);
            }
            count += 1;
        }
        self.published_revision = self.presentation_revision;
        self.published_layout = Some(layout.clone());
        count
    }
    pub fn identity(&self) -> u64 {
        self.identity
    }

    /// Constant-time observation; does not consume layout/paint/hit dirty queues.
    pub fn presentation_key(&self) -> Option<(u64, u64)> {
        (!self.disposed).then_some((self.identity, self.presentation_revision))
    }

    /// Input identity for binding/style resolution; post-layout translation is independent.
    pub(crate) fn content_key(&self) -> Option<(u64, u64)> {
        (!self.disposed).then_some((self.identity, self.content_revision))
    }

    pub(crate) fn binding_set_key(&self) -> Option<(u64, u64)> {
        (!self.disposed).then_some((self.identity, self.structure_revision))
    }

    #[cfg(test)]
    pub(crate) fn presentation_properties(
        &self,
    ) -> impl Iterator<Item = (&str, UiProperty, UiValue)> {
        self.presentation_properties_with_slots()
            .map(|(_, node, property, value)| (node, property, value))
    }

    pub(crate) fn presentation_properties_with_slots(
        &self,
    ) -> impl Iterator<Item = (u32, &str, UiProperty, UiValue)> {
        self.presentation_properties
            .iter()
            .copied()
            .chain(
                self.animation_values
                    .keys()
                    .copied()
                    .filter(|key| !self.presentation_properties.contains(key)),
            )
            .filter_map(move |(slot, property)| {
                let node = self
                    .nodes
                    .get(slot as usize)?
                    .alive
                    .then_some(&self.nodes[slot as usize])?;
                Some((
                    slot,
                    node.source_node.node_id.as_str(),
                    property,
                    self.presentation_value(slot, property),
                ))
            })
    }

    /// None means the reader must rebuild from the committed instance. A read never drains work
    /// needed by layout, hit testing, or another presentation consumer.
    pub(crate) fn presentation_changes_since(
        &self,
        revision: u64,
    ) -> Option<Vec<(u32, AuiPresentationChange)>> {
        if self.disposed || revision < self.presentation_change_floor {
            return None;
        }
        if revision == self.presentation_revision {
            return Some(Vec::new());
        }
        let mut expected = self.presentation_revision;
        let mut seen = false;
        let mut changed = BTreeSet::new();
        for &(change_revision, slot, property) in self
            .presentation_changes
            .iter()
            .rev()
            .take_while(|(change_revision, _, _)| *change_revision > revision)
        {
            if !seen && change_revision != expected {
                return None;
            }
            seen = true;
            if change_revision != expected {
                if change_revision.checked_add(1)? != expected {
                    return None;
                }
                expected = change_revision;
            }
            changed.insert((slot, property));
        }
        (seen && expected == revision.checked_add(1)?).then(|| changed.into_iter().collect())
    }

    pub(crate) fn presentation_property_at(
        &self,
        slot: u32,
        property: UiProperty,
    ) -> Option<(&str, UiValue)> {
        if !self.presentation_properties.contains(&(slot, property))
            && !self.animation_values.contains_key(&(slot, property))
        {
            return None;
        }
        let node = self
            .nodes
            .get(slot as usize)?
            .alive
            .then_some(&self.nodes[slot as usize])?;
        Some((
            node.source_node.node_id.as_str(),
            self.presentation_value(slot, property),
        ))
    }

    pub(crate) fn presentation_node_id_at(&self, slot: u32) -> Option<&str> {
        self.nodes
            .get(slot as usize)
            .filter(|node| node.alive)
            .map(|node| node.source_node.node_id.as_str())
    }

    fn record_presentation_change(&mut self, slot: u32, property: UiProperty) {
        self.record_node_change(slot, AuiPresentationChange::Property(property));
    }

    fn record_node_change(&mut self, slot: u32, change: AuiPresentationChange) {
        self.presentation_changes
            .push_back((self.presentation_revision, slot, change));
        while self.presentation_changes.len() > MAX_PRESENTATION_CHANGES {
            if let Some((revision, _, _)) = self.presentation_changes.pop_front() {
                self.presentation_change_floor = revision;
            }
        }
    }

    /// Apply a decoded module batch only after its containing callback has been accepted.
    /// The existing edit owner validates every live handle and binding before any value is committed.
    pub fn apply_update_batch(
        &mut self,
        batch: &project_runtime_sdk::aui_wire::UiUpdateBatch,
    ) -> Result<usize, UiError> {
        let mut edit = self.edit();
        for update in batch.ownership_updates() {
            edit.transfer_property_ownership(update.handle, update.property, update.owner)?;
        }
        for update in batch.updates() {
            if update.notify {
                edit.write(update.handle, update.property, update.value.clone())?;
            } else {
                edit.write_without_notify(update.handle, update.property, update.value.clone())?;
            }
        }
        edit.commit()
    }

    pub fn mount(source: Arc<AuiDocument>) -> Result<Self, Vec<AuiValidationItem>> {
        // Run full structural validation even for legacy authoring, without modifying its schema.
        let mut validation_source = (*source).clone();
        if matches!(
            validation_source.schema_version.as_str(),
            crate::aui::AUI_DOCUMENT_SCHEMA_VERSION
                | crate::aui::LEGACY_AUI_DOCUMENT_SCHEMA_VERSION
        ) && !aui_layout_schema::uses_c2(&validation_source)
        {
            validation_source.schema_version = aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
        }
        let mut errors = aui_layout_schema::validate(&validation_source);
        for node in &validation_source.nodes {
            if node.kind != AuiNodeKind::Custom {
                continue;
            }
            match node.custom.as_ref() {
                Some(custom)
                    if !custom.type_id.is_empty()
                        && custom.type_id.len()
                            <= project_runtime_sdk::aui_extension::MAX_UI_EXTENSION_IDENTIFIER_BYTES
                        && custom.schema_version > 0
                        && custom.properties.len()
                            <= project_runtime_sdk::aui_extension::MAX_UI_EXTENSION_PROPERTIES => {}
                _ => errors.push(AuiValidationItem::error(
                    "aui_custom_node_schema_invalid",
                    Some(node.node_id.clone()),
                    None,
                    "Custom AUI nodes require a stable typeId, non-zero schemaVersion and bounded typed properties.",
                    "Declare the matching Project Game SDK UI extension type.",
                )),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let identity = NEXT_INSTANCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| {
                vec![AuiValidationItem::error(
                    "aui_instance_identity_exhausted",
                    None,
                    None,
                    "UI instance identity space is exhausted.",
                    "Restart the runtime session.",
                )]
            })?;
        let mut nodes = Vec::with_capacity(source.nodes.len());
        let mut by_id = HashMap::with_capacity(source.nodes.len());
        let mut dirty = Vec::with_capacity(source.nodes.len());
        let mut custom_dirty = BTreeMap::new();
        for (index, node) in source.nodes.iter().enumerate() {
            let binding_owned = binding_ownership(&node.binding_refs);
            let visibility = match node.layout_visibility {
                Some(LayoutVisibility::Collapsed) => UiVisibility::Collapsed,
                Some(LayoutVisibility::Hidden) => UiVisibility::Hidden,
                _ if !node.visible => UiVisibility::Hidden,
                _ => UiVisibility::Visible,
            };
            nodes.push(NodeState {
                source_node: node.clone(),
                alive: true,
                kind: match node.kind {
                    AuiNodeKind::Text => UiNodeType::Text,
                    AuiNodeKind::Button => UiNodeType::Button,
                    AuiNodeKind::Slider => UiNodeType::Slider,
                    _ => UiNodeType::Node,
                },
                generation: 1,
                text: node.text.clone().unwrap_or_default(),
                value: crate::aui::normalize_slider_value(
                    node,
                    node.slider_value.unwrap_or(node.slider_min.unwrap_or(0.0)),
                ),
                translation: node.presentation.unwrap_or_default().transform.translation,
                presentation: node.presentation.unwrap_or_default(),
                resource: node
                    .image
                    .as_ref()
                    .map(|image| UiResourceRef {
                        asset_id: image.asset_id.clone(),
                        guid: source
                            .resource_identities
                            .get(&image.asset_id)
                            .map_or_else(String::new, |(guid, _)| guid.clone()),
                    })
                    .or_else(|| graphics_resource(node)),
                local_style: UiLocalStyle::default(),
                interactable: node.interactable,
                visibility,
                active: true,
                active_in_hierarchy: true,
                parent: None,
                children: Vec::new(),
                canvas_root: index as u32,
                template_instance_id: None,
                binding_owned,
                dirty: AuiInvalidation {
                    style: true,
                    measure: true,
                    layout: true,
                    paint: true,
                    transform: true,
                    hit: true,
                },
                queued: true,
            });
            by_id.insert(node.node_id.clone(), index as u32);
            dirty.push(index as u32);
            if node.kind == AuiNodeKind::Custom {
                custom_dirty.insert(index as u32, crate::aui_extension::AuiCustomDirty::all());
            }
        }
        for (slot, node) in source.nodes.iter().enumerate() {
            let Some(parent_id) = node.parent.as_ref() else {
                continue;
            };
            let parent = *by_id
                .get(parent_id)
                .expect("validated AUI parent must resolve");
            nodes[slot].parent = Some(parent);
            nodes[parent as usize].children.push(slot as u32);
        }
        let canvas_roots = source
            .canvases
            .iter()
            .filter_map(|canvas| by_id.get(&canvas.root_node).copied())
            .collect::<BTreeSet<_>>();
        for slot in 0..nodes.len() {
            let mut root = slot as u32;
            while let Some(parent) = nodes[root as usize].parent {
                root = parent;
            }
            nodes[slot].canvas_root = root;
        }
        Ok(Self {
            source,
            identity,
            nodes,
            by_id,
            dirty,
            disposed: false,
            layouts: HashMap::new(),
            layout_source: None,
            animation_values: BTreeMap::new(),
            base_revisions: BTreeMap::new(),
            presentation_revision: 0,
            content_revision: 0,
            structure_revision: 0,
            presentation_properties: BTreeSet::new(),
            presentation_changes: VecDeque::new(),
            presentation_change_floor: 0,
            subscriptions: BTreeMap::new(),
            next_subscription_id: 1,
            notifications: VecDeque::new(),
            dispatch: None,
            next_notification_sequence: 1,
            canvas_roots,
            next_template_instance_id: 1,
            committed_geometry: HashMap::new(),
            published_layout: None,
            published_revision: 0,
            published_matrices: HashMap::new(),
            pending_keep_screen: BTreeMap::new(),
            retired_handles: BTreeSet::new(),
            custom_dirty,
        })
    }

    pub fn source(&self) -> &AuiDocument {
        &self.source
    }

    /// Base layout before post-layout transforms. Source validation occurs only at mount.
    /// The measurer's external font/style inputs must stay stable for this mounted layout.
    /// Failures preserve the previous snapshot; they do not publish partially computed geometry.
    pub fn layout(
        &mut self,
        canvas: &str,
        measure: &mut dyn AuiIntrinsicMeasure,
    ) -> Result<&AuiLayoutSnapshot, Vec<AuiValidationItem>> {
        if self.disposed {
            return Err(vec![AuiValidationItem::error(
                "aui_instance_disposed",
                None,
                None,
                "Cannot layout a disposed instance.",
                "Mount a new instance.",
            )]);
        }
        if !self.layouts.contains_key(canvas) {
            let shared = self.layout_source.get_or_insert_with(|| {
                Arc::new(crate::aui_layout_runtime::AuiLayoutSource::new(
                    &self.source,
                ))
            });
            let mut solver = AuiLayoutSolver::for_shared_instance(&self.source, canvas, shared)?;
            // A canvas may first be consumed after several accepted callbacks.
            for (slot, node) in self.nodes.iter().enumerate() {
                solver.update_instance_node(
                    slot,
                    (node.kind == UiNodeType::Text).then_some(node.text.as_str()),
                    effective_layout_visibility(node.active_in_hierarchy, node.visibility),
                );
            }
            self.layouts.insert(canvas.into(), solver);
        }
        let geometry = {
            let snapshot = self
                .layouts
                .get_mut(canvas)
                .expect("mounted canvas")
                .solve_instance(measure)?;
            snapshot
                .nodes
                .iter()
                .map(|node| (node.node_id.clone(), node.rect))
                .collect::<HashMap<_, _>>()
        };
        let slots = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.alive && geometry.contains_key(&node.source_node.node_id))
            .map(|(slot, _)| slot as u32)
            .collect::<Vec<_>>();
        for slot in slots {
            self.committed_geometry.insert(
                slot,
                geometry[&self.nodes[slot as usize].source_node.node_id],
            );
        }
        self.apply_pending_keep_screen(canvas);
        Ok(self
            .layouts
            .get(canvas)
            .and_then(AuiLayoutSolver::instance_snapshot)
            .expect("solved canvas snapshot"))
    }

    pub fn layout_snapshot(&self, canvas: &str) -> Option<&AuiLayoutSnapshot> {
        self.layouts
            .get(canvas)
            .and_then(AuiLayoutSolver::instance_snapshot)
    }

    pub fn layout_stats(&self, canvas: &str) -> Option<AuiLayoutStats> {
        self.layouts.get(canvas).map(AuiLayoutSolver::last_stats)
    }

    pub fn node(&self, id: &str) -> Result<UiNodeHandle, UiError> {
        if self.disposed {
            return Err(error(UiErrorKind::Disposed, None, None));
        }
        self.by_id
            .get(id)
            .map(|&slot| self.handle(slot))
            .ok_or_else(|| error(UiErrorKind::UnknownNode, None, None))
    }

    pub(crate) fn transaction_draft(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
            identity: self.identity,
            nodes: self.nodes.clone(),
            by_id: self.by_id.clone(),
            dirty: self.dirty.clone(),
            disposed: self.disposed,
            layouts: HashMap::new(),
            layout_source: self.layout_source.clone(),
            presentation_revision: self.presentation_revision,
            animation_values: self.animation_values.clone(),
            base_revisions: self.base_revisions.clone(),
            content_revision: self.content_revision,
            structure_revision: self.structure_revision,
            presentation_properties: self.presentation_properties.clone(),
            presentation_changes: self.presentation_changes.clone(),
            presentation_change_floor: self.presentation_change_floor,
            subscriptions: self.subscriptions.clone(),
            next_subscription_id: self.next_subscription_id,
            notifications: self.notifications.clone(),
            dispatch: self.dispatch.clone(),
            next_notification_sequence: self.next_notification_sequence,
            canvas_roots: self.canvas_roots.clone(),
            next_template_instance_id: self.next_template_instance_id,
            committed_geometry: self.committed_geometry.clone(),
            published_layout: self.published_layout.clone(),
            published_revision: self.published_revision,
            published_matrices: self.published_matrices.clone(),
            pending_keep_screen: self.pending_keep_screen.clone(),
            retired_handles: self.retired_handles.clone(),
            custom_dirty: self.custom_dirty.clone(),
        }
    }

    /// Move cached layout only after the structural transaction has succeeded. Unchanged
    /// Canvas roots retain their caches; changed topology is rebuilt on its next layout.
    pub(crate) fn retain_layouts_from(&mut self, previous: &mut Self) {
        if self.identity != previous.identity || previous.layouts.is_empty() || self.disposed {
            return;
        }
        let source = Arc::new(AuiLayoutSource::new(&self.source));
        let mut layouts = std::mem::take(&mut previous.layouts);
        layouts.retain(|canvas, solver| {
            if !solver.reconcile_source_slots(&self.source, &source.node_slots) {
                return false;
            }
            for (slot, node) in self.nodes.iter().enumerate() {
                solver.update_instance_source_node(slot, source.node(slot));
                solver.update_instance_node(
                    slot,
                    (node.kind == UiNodeType::Text).then_some(node.text.as_str()),
                    effective_layout_visibility(node.active_in_hierarchy, node.visibility),
                );
            }
            let Some(canvas) = self.source.canvases.iter().find(|c| &c.canvas_id == canvas) else {
                return false;
            };
            solver.update_instance_root_rect(canvas.resolved_layout_root_rect());
            solver.configure_instance_inputs().is_ok()
        });
        self.layouts = layouts;
        self.layout_source = Some(source);
    }

    pub(crate) fn instantiate_template(
        &mut self,
        template: &AuiDocument,
        parent: UiNodeHandle,
        sibling_index: Option<u32>,
    ) -> Result<UiTemplateInstance, UiError> {
        let parent_slot = self.live_slot(parent)?;
        if template.canvases.len() != 1 || template.nodes.is_empty() {
            return Err(error(UiErrorKind::InvalidValue, Some(parent), None));
        }
        let root_local_id = &template.canvases[0].root_node;
        let local_slots = template
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.node_id.as_str(), slot))
            .collect::<HashMap<_, _>>();
        let template_root = *local_slots
            .get(root_local_id.as_str())
            .ok_or_else(|| error(UiErrorKind::InvalidValue, Some(parent), None))?;
        let template_instance_id = self.next_template_instance_id;
        self.next_template_instance_id = self
            .next_template_instance_id
            .checked_add(1)
            .ok_or_else(|| error(UiErrorKind::CapacityExceeded, Some(parent), None))?;
        let canvas_root = self.nodes[parent_slot].canvas_root;
        let mut allocated = Vec::with_capacity(template.nodes.len());
        for source in &template.nodes {
            let mut runtime_source = source.clone();
            runtime_source.node_id =
                format!("__template_{template_instance_id}__{}", source.node_id);
            let slot = self.allocate_dynamic_node(
                runtime_source,
                canvas_root,
                Some(template_instance_id),
            )?;
            allocated.push(slot);
        }
        for (local_slot, source) in template.nodes.iter().enumerate() {
            let slot = allocated[local_slot];
            let parent = if local_slot == template_root {
                Some(parent_slot as u32)
            } else {
                source
                    .parent
                    .as_deref()
                    .and_then(|id| local_slots.get(id).copied())
                    .map(|parent| allocated[parent] as u32)
            };
            let children = source
                .children
                .iter()
                .filter_map(|id| local_slots.get(id.as_str()).copied())
                .map(|child| allocated[child] as u32)
                .collect::<Vec<_>>();
            self.nodes[slot].parent = parent;
            self.nodes[slot].children = children;
        }
        let root_slot = allocated[template_root] as u32;
        insert_child(
            &mut self.nodes[parent_slot].children,
            root_slot,
            sibling_index,
        );
        self.recompute_active_subtrees(&[root_slot]);
        self.refresh_structure()?;
        Ok(UiTemplateInstance {
            template_instance_id,
            root: self.handle(root_slot),
            nodes: template
                .nodes
                .iter()
                .enumerate()
                .map(|(slot, node)| UiTemplateNodeRef {
                    local_id: node.node_id.clone(),
                    handle: self.handle(allocated[slot] as u32),
                })
                .collect(),
        })
    }

    pub(crate) fn delete_subtree(&mut self, target: UiNodeHandle) -> Result<(), UiError> {
        let target_slot = self.live_slot(target)?;
        if self.canvas_roots.contains(&(target_slot as u32)) {
            return Err(error(UiErrorKind::IncompatibleRoot, Some(target), None));
        }
        let mut stack = vec![target_slot as u32];
        let mut subtree = Vec::new();
        while let Some(slot) = stack.pop() {
            stack.extend(self.nodes[slot as usize].children.iter().copied());
            subtree.push(slot);
        }
        if let Some(parent) = self.nodes[target_slot].parent {
            self.nodes[parent as usize]
                .children
                .retain(|child| *child != target_slot as u32);
        }
        for &slot in &subtree {
            let handle = self.handle(slot);
            let active = self.nodes[slot as usize].active_in_hierarchy;
            if active {
                self.queue_notification(handle, AuiNotificationKind::Disable, None);
            }
            self.queue_notification(handle, AuiNotificationKind::Dispose, None);
            self.retired_handles.insert(handle);
        }
        for slot in subtree {
            let root = self.nodes[slot as usize].canvas_root;
            let runtime_id = self.nodes[slot as usize].source_node.node_id.clone();
            self.by_id.remove(&runtime_id);
            let generation = next_dynamic_generation()?;
            let mut tombstone = AuiNode::new(
                format!("__dead_{slot}_{generation}"),
                AuiNodeKind::Panel,
                AuiRect::fixed_position(0.0, 0.0, 0.0, 0.0),
            );
            tombstone.visible = false;
            tombstone.layout_visibility = Some(LayoutVisibility::Collapsed);
            let state = &mut self.nodes[slot as usize];
            state.source_node = tombstone;
            state.alive = false;
            state.generation = generation;
            state.text.clear();
            state.translation = [0.0; 2];
            state.interactable = false;
            state.visibility = UiVisibility::Collapsed;
            state.active = false;
            state.active_in_hierarchy = false;
            state.parent = Some(root);
            state.children.clear();
            state.template_instance_id = None;
            state.binding_owned = [false; 13];
            state.dirty = AuiInvalidation::default();
            state.queued = false;
            self.nodes[root as usize].children.push(slot);
            self.presentation_properties
                .retain(|(property_slot, _)| *property_slot != slot);
            self.animation_values
                .retain(|(property_slot, _), _| *property_slot != slot);
            self.base_revisions
                .retain(|(property_slot, _), _| *property_slot != slot);
            self.pending_keep_screen.remove(&slot);
            self.committed_geometry.remove(&slot);
        }
        self.refresh_structure()
    }

    pub(crate) fn reparent(
        &mut self,
        target: UiNodeHandle,
        new_parent: UiNodeHandle,
        sibling_index: Option<u32>,
        mode: UiReparentMode,
    ) -> Result<(), UiError> {
        let target_slot = self.live_slot(target)?;
        let parent_slot = self.live_slot(new_parent)?;
        if self.canvas_roots.contains(&(target_slot as u32)) {
            return Err(error(UiErrorKind::IncompatibleRoot, Some(target), None));
        }
        if target_slot == parent_slot || self.is_descendant(parent_slot as u32, target_slot as u32)
        {
            return Err(error(UiErrorKind::HierarchyCycle, Some(target), None));
        }
        if self.nodes[target_slot].canvas_root != self.nodes[parent_slot].canvas_root {
            return Err(error(UiErrorKind::IncompatibleRoot, Some(new_parent), None));
        }
        let keep_screen = if mode == UiReparentMode::KeepScreen {
            use crate::aui_layout_schema::LayoutMode;
            let parent_mode = self.nodes[parent_slot]
                .source_node
                .layout
                .as_ref()
                .map_or(LayoutMode::Anchor, |layout| layout.mode);
            if parent_mode != LayoutMode::Anchor {
                return Err(error(UiErrorKind::ControlledLayout, Some(new_parent), None));
            }
            self.final_matrix(parent_slot as u32)?
                .try_inverse()
                .map_err(|_| error(UiErrorKind::NonInvertibleTransform, Some(new_parent), None))?;
            Some(self.local_to_screen(target, [0.0, 0.0])?)
        } else {
            None
        };
        if let Some(parent) = self.nodes[target_slot].parent {
            self.nodes[parent as usize]
                .children
                .retain(|child| *child != target_slot as u32);
        }
        self.nodes[target_slot].parent = Some(parent_slot as u32);
        insert_child(
            &mut self.nodes[parent_slot].children,
            target_slot as u32,
            sibling_index,
        );
        if let Some(screen) = keep_screen {
            self.pending_keep_screen.insert(target_slot as u32, screen);
        }
        self.recompute_active_subtrees(&[target_slot as u32]);
        self.refresh_structure()
    }

    pub(crate) fn set_sibling_index(
        &mut self,
        target: UiNodeHandle,
        sibling_index: u32,
    ) -> Result<(), UiError> {
        let target_slot = self.live_slot(target)?;
        let parent = self.nodes[target_slot]
            .parent
            .ok_or_else(|| error(UiErrorKind::IncompatibleRoot, Some(target), None))?;
        let children = &mut self.nodes[parent as usize].children;
        children.retain(|child| *child != target_slot as u32);
        insert_child(children, target_slot as u32, Some(sibling_index));
        self.refresh_structure()
    }

    pub(crate) fn local_to_screen(
        &self,
        target: UiNodeHandle,
        point: [f32; 2],
    ) -> Result<[f32; 2], UiError> {
        if !point.iter().all(|value| value.is_finite()) {
            return Err(error(UiErrorKind::InvalidValue, Some(target), None));
        }
        let slot = self.live_slot(target)? as u32;
        self.final_matrix(slot)?
            .transform_point(point)
            .map_err(|_| error(UiErrorKind::InvalidValue, Some(target), None))
    }

    pub(crate) fn screen_to_local(
        &self,
        target: UiNodeHandle,
        point: [f32; 2],
    ) -> Result<[f32; 2], UiError> {
        let slot = self.live_slot(target)? as u32;
        if !point.iter().all(|value| value.is_finite()) {
            return Err(error(UiErrorKind::InvalidValue, Some(target), None));
        }
        self.final_matrix(slot)?
            .try_inverse()
            .map_err(|_| error(UiErrorKind::NonInvertibleTransform, Some(target), None))?
            .transform_point(point)
            .map_err(|_| error(UiErrorKind::InvalidValue, Some(target), None))
    }

    fn final_matrix(
        &self,
        slot: u32,
    ) -> Result<project_runtime_sdk::aui_presentation::Affine2D, UiError> {
        if self.published_revision == self.presentation_revision {
            if let Some(matrix) = self.published_matrices.get(&slot) {
                return Ok(*matrix);
            }
        }
        let node = &self.nodes[slot as usize];
        let handle = UiNodeHandle {
            instance: self.identity,
            slot,
            generation: node.generation,
        };
        let rect = self
            .committed_geometry
            .get(&slot)
            .ok_or_else(|| error(UiErrorKind::LayoutUnavailable, Some(handle), None))?;
        let (parent, origin) = if let Some(parent) = node.parent {
            let parent_rect = self
                .committed_geometry
                .get(&parent)
                .ok_or_else(|| error(UiErrorKind::LayoutUnavailable, Some(handle), None))?;
            (
                self.final_matrix(parent)?,
                [rect.x - parent_rect.x, rect.y - parent_rect.y],
            )
        } else {
            (
                project_runtime_sdk::aui_presentation::Affine2D::IDENTITY,
                [rect.x, rect.y],
            )
        };
        let mut transform = node.presentation.transform;
        transform.translation = node.translation;
        let mut presentation = node.presentation;
        presentation.transform = transform;
        for (&(key_slot, property), value) in self
            .animation_values
            .range((slot, UiProperty::Text)..=(slot, UiProperty::LocalStyle))
        {
            if key_slot == slot {
                presentation.apply(property, value);
            }
        }
        transform = presentation.transform;
        parent
            .compose(
                transform
                    .local_matrix(origin, [rect.width, rect.height])
                    .map_err(|_| error(UiErrorKind::InvalidValue, Some(handle), None))?,
            )
            .map_err(|_| error(UiErrorKind::InvalidValue, Some(handle), None))
    }

    fn live_slot(&self, handle: UiNodeHandle) -> Result<usize, UiError> {
        self.state(handle)?;
        Ok(handle.slot as usize)
    }

    fn allocate_dynamic_node(
        &mut self,
        source_node: AuiNode,
        canvas_root: u32,
        template_instance_id: Option<u64>,
    ) -> Result<usize, UiError> {
        let generation = next_dynamic_generation()?;
        let state = node_state(source_node, generation, canvas_root, template_instance_id);
        if let Some(slot) = self.nodes.iter().position(|node| !node.alive) {
            if let Some(parent) = self.nodes[slot].parent {
                self.nodes[parent as usize]
                    .children
                    .retain(|child| *child != slot as u32);
            }
            self.nodes[slot] = state;
            Ok(slot)
        } else {
            if self.nodes.len() >= crate::aui_layout_schema::MAX_LAYOUT_NODES {
                return Err(error(UiErrorKind::CapacityExceeded, None, None));
            }
            self.nodes.push(state);
            Ok(self.nodes.len() - 1)
        }
    }

    fn is_descendant(&self, candidate: u32, ancestor: u32) -> bool {
        let mut cursor = Some(candidate);
        while let Some(slot) = cursor {
            if slot == ancestor {
                return true;
            }
            cursor = self.nodes[slot as usize].parent;
        }
        false
    }

    fn accumulated_translation(&self, slot: u32, include_self: bool) -> [f32; 2] {
        let mut result = [0.0, 0.0];
        let mut cursor = if include_self {
            Some(slot)
        } else {
            self.nodes[slot as usize].parent
        };
        while let Some(current) = cursor {
            let translation = match self.presentation_value(current, UiProperty::Translation) {
                UiValue::Translation(value) => value,
                _ => unreachable!("translation property"),
            };
            result[0] += translation[0];
            result[1] += translation[1];
            cursor = self.nodes[current as usize].parent;
        }
        result
    }

    fn apply_pending_keep_screen(&mut self, canvas: &str) {
        let Some(root) = self
            .source
            .canvases
            .iter()
            .find(|candidate| candidate.canvas_id == canvas)
            .and_then(|canvas| self.by_id.get(&canvas.root_node))
            .copied()
        else {
            return;
        };
        let pending = self
            .pending_keep_screen
            .iter()
            .filter(|(slot, _)| self.nodes[**slot as usize].canvas_root == root)
            .map(|(slot, point)| (*slot, *point))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return;
        }
        for (slot, desired) in pending {
            let Some(rect) = self.committed_geometry.get(&slot).copied() else {
                continue;
            };
            let parent = self.nodes[slot as usize].parent;
            let parent_matrix = match parent {
                Some(p) => self.final_matrix(p),
                None => Ok(project_runtime_sdk::aui_presentation::Affine2D::IDENTITY),
            };
            let Ok(parent_inverse) = parent_matrix.and_then(|m| {
                m.try_inverse()
                    .map_err(|_| error(UiErrorKind::NonInvertibleTransform, None, None))
            }) else {
                continue;
            };
            let Ok(desired_local) = parent_inverse.transform_point(desired) else {
                continue;
            };
            let parent_rect = parent
                .and_then(|p| self.committed_geometry.get(&p))
                .copied()
                .unwrap_or_default();
            let mut transform = self.nodes[slot as usize].presentation.transform;
            transform.translation = [0.; 2];
            let Ok(origin) = transform
                .local_matrix(
                    [rect.x - parent_rect.x, rect.y - parent_rect.y],
                    [rect.width, rect.height],
                )
                .and_then(|m| m.transform_point([0.; 2]))
            else {
                continue;
            };
            let translation = [desired_local[0] - origin[0], desired_local[1] - origin[1]];
            let node = &mut self.nodes[slot as usize];
            node.translation = translation;
            node.dirty.merge(AuiInvalidation {
                transform: true,
                hit: true,
                ..Default::default()
            });
            if !node.queued {
                node.queued = true;
                self.dirty.push(slot);
            }
            self.presentation_properties
                .insert((slot, UiProperty::Translation));
            self.pending_keep_screen.remove(&slot);
        }
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        let changed = self
            .presentation_properties
            .iter()
            .filter_map(|(slot, property)| (*property == UiProperty::Translation).then_some(*slot))
            .collect::<Vec<_>>();
        for slot in changed {
            self.record_presentation_change(slot, UiProperty::Translation);
        }
    }

    fn refresh_structure(&mut self) -> Result<(), UiError> {
        let mut document = (*self.source).clone();
        let runtime_ids = self
            .nodes
            .iter()
            .map(|node| node.source_node.node_id.clone())
            .collect::<Vec<_>>();
        for (slot, state) in self.nodes.iter_mut().enumerate() {
            state.source_node.parent = state
                .parent
                .map(|parent| runtime_ids[parent as usize].clone());
            state.source_node.children = state
                .children
                .iter()
                .map(|child| runtime_ids[*child as usize].clone())
                .collect();
            if state.alive {
                self.by_id
                    .insert(state.source_node.node_id.clone(), slot as u32);
            }
        }
        document.nodes = self
            .nodes
            .iter()
            .map(|node| node.source_node.clone())
            .collect();
        let diagnostics = aui_layout_schema::validate(&document);
        if !diagnostics.is_empty() {
            return Err(error(UiErrorKind::InvalidValue, None, None));
        }
        self.source = Arc::new(document);
        self.layouts.clear();
        self.layout_source = None;
        self.structure_revision = self.structure_revision.saturating_add(1);
        self.content_revision = self.content_revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        self.dirty.clear();
        self.custom_dirty.clear();
        for (slot, node) in self.nodes.iter_mut().enumerate() {
            if !node.alive {
                continue;
            }
            node.dirty.merge(AuiInvalidation {
                style: true,
                measure: true,
                layout: true,
                paint: true,
                transform: true,
                hit: true,
            });
            node.queued = true;
            self.dirty.push(slot as u32);
            if node.source_node.kind == AuiNodeKind::Custom {
                self.custom_dirty
                    .insert(slot as u32, crate::aui_extension::AuiCustomDirty::all());
            }
        }
        Ok(())
    }

    pub fn edit(&mut self) -> AuiEdit<'_> {
        self.edit_from(AuiChangeOrigin::Programmatic)
    }

    pub fn edit_from(&mut self, origin: AuiChangeOrigin) -> AuiEdit<'_> {
        AuiEdit {
            instance: self,
            pending: BTreeMap::new(),
            pending_ownership: BTreeMap::new(),
            text_bytes: 0,
            failure: None,
            origin,
            default_notify: true,
        }
    }

    pub fn edit_without_notify(&mut self, origin: AuiChangeOrigin) -> AuiEdit<'_> {
        AuiEdit {
            instance: self,
            pending: BTreeMap::new(),
            pending_ownership: BTreeMap::new(),
            text_bytes: 0,
            failure: None,
            origin,
            default_notify: false,
        }
    }

    /// Reuses caller and owner buffers; a clean drain visits no nodes and allocates nothing.
    pub fn drain_invalidations(&mut self, output: &mut Vec<AuiInvalidationEntry>) {
        output.clear();
        for slot in self.dirty.drain(..) {
            let node = &mut self.nodes[slot as usize];
            output.push(AuiInvalidationEntry {
                handle: UiNodeHandle {
                    instance: self.identity,
                    slot,
                    generation: node.generation,
                },
                invalidation: std::mem::take(&mut node.dirty),
            });
            node.queued = false;
        }
    }

    pub(crate) fn custom_work(
        &self,
    ) -> Vec<(
        crate::aui_extension::AuiCustomRuntimeNode,
        crate::aui_extension::AuiCustomDirty,
    )> {
        self.custom_dirty
            .iter()
            .filter_map(|(&slot, &dirty)| {
                self.custom_runtime_node_at(slot).map(|node| (node, dirty))
            })
            .collect()
    }

    pub(crate) fn has_custom_work(&self) -> bool {
        !self.custom_dirty.is_empty()
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
        let slot = self.live_slot(handle)? as u32;
        self.custom_runtime_node_at(slot)
            .map(|node| {
                let dirty = self.custom_dirty.get(&slot).copied().unwrap_or_default();
                (node, dirty)
            })
            .ok_or_else(|| error(UiErrorKind::WrongNodeType, Some(handle), None))
    }

    fn custom_runtime_node_at(
        &self,
        slot: u32,
    ) -> Option<crate::aui_extension::AuiCustomRuntimeNode> {
        let node = self
            .nodes
            .get(slot as usize)?
            .alive
            .then_some(&self.nodes[slot as usize])?;
        let spec = node.source_node.custom.clone()?;
        let rect = node.source_node.rect;
        Some(crate::aui_extension::AuiCustomRuntimeNode {
            handle: self.handle(slot),
            node_id: node.source_node.node_id.clone(),
            spec,
            rect: [
                rect.offset_min.x,
                rect.offset_min.y,
                rect.size.x,
                rect.size.y,
            ],
            children: node
                .children
                .iter()
                .map(|&child| self.handle(child))
                .collect(),
            runtime: node.source_node.custom_runtime.clone().unwrap_or_default(),
        })
    }

    pub(crate) fn custom_runtime_outputs(
        &self,
    ) -> impl Iterator<Item = (&str, &crate::aui_extension::AuiCustomRuntimeOutput)> {
        self.nodes.iter().filter_map(|node| {
            node.alive.then_some(node).and_then(|node| {
                node.source_node
                    .custom_runtime
                    .as_ref()
                    .map(|output| (node.source_node.node_id.as_str(), output))
            })
        })
    }

    pub(crate) fn invalidate_custom(
        &mut self,
        handle: UiNodeHandle,
        kind: project_runtime_sdk::aui_host::UiInvalidateKind,
    ) -> Result<(), UiError> {
        let slot = self.live_slot(handle)? as u32;
        if self.nodes[slot as usize].source_node.kind != AuiNodeKind::Custom
            || self.nodes[slot as usize].source_node.custom.is_none()
        {
            return Err(error(UiErrorKind::WrongNodeType, Some(handle), None));
        }
        let dirty = self.custom_dirty.entry(slot).or_default();
        match kind {
            project_runtime_sdk::aui_host::UiInvalidateKind::Measure => {
                dirty.measure = true;
                dirty.layout = true;
                dirty.paint = true;
            }
            project_runtime_sdk::aui_host::UiInvalidateKind::Paint => dirty.paint = true,
        }
        Ok(())
    }

    pub(crate) fn is_custom_node(&self, handle: UiNodeHandle) -> bool {
        self.state(handle).is_ok_and(|node| {
            node.source_node.kind == AuiNodeKind::Custom && node.source_node.custom.is_some()
        })
    }

    pub(crate) fn validate_custom_property(
        &self,
        handle: UiNodeHandle,
        name: &str,
        value: &project_runtime_sdk::aui_extension::UiExtensionValue,
        descriptor: &project_runtime_sdk::aui_extension::UiExtensionTypeDescriptor,
    ) -> Result<(), UiError> {
        use project_runtime_sdk::aui_extension::{
            UiExtensionPropertyType as Type, UiExtensionValue as Value,
        };
        let node = self.state(handle)?;
        let Some(spec) = node.source_node.custom.as_ref() else {
            return Err(error(UiErrorKind::WrongNodeType, Some(handle), None));
        };
        if spec.type_id != descriptor.type_id || spec.schema_version != descriptor.schema_version {
            return Err(error(UiErrorKind::InvalidValue, Some(handle), None));
        }
        let Some(property) = descriptor
            .properties
            .iter()
            .find(|property| property.name == name)
        else {
            return Err(error(UiErrorKind::InvalidValue, Some(handle), None));
        };
        let actual = match value {
            Value::Bool(_) => Type::Bool,
            Value::Float(value) if value.is_finite() => Type::Float,
            Value::Integer(_) => Type::Integer,
            Value::Text(_) => Type::Text,
            Value::Color(value) if value.iter().all(|channel| channel.is_finite()) => Type::Color,
            Value::Asset(value) if !value.is_empty() => Type::Asset,
            _ => return Err(error(UiErrorKind::InvalidValue, Some(handle), None)),
        };
        if actual != property.property_type {
            return Err(error(UiErrorKind::WrongValueType, Some(handle), None));
        }
        Ok(())
    }

    pub(crate) fn apply_custom_property(
        &mut self,
        handle: UiNodeHandle,
        name: &str,
        value: project_runtime_sdk::aui_extension::UiExtensionValue,
        descriptor: &project_runtime_sdk::aui_extension::UiExtensionTypeDescriptor,
    ) -> Result<bool, UiError> {
        use project_runtime_sdk::aui_extension::UiExtensionInvalidation as Kind;
        self.validate_custom_property(handle, name, &value, descriptor)?;
        let slot = self.live_slot(handle)?;
        let property = descriptor
            .properties
            .iter()
            .find(|property| property.name == name)
            .expect("validated custom property");
        let state = &mut self.nodes[slot];
        let spec = state
            .source_node
            .custom
            .as_mut()
            .expect("validated custom node");
        if spec.properties.get(name) == Some(&value) {
            return Ok(false);
        }
        spec.properties.insert(name.to_string(), value);
        let custom_dirty = self.custom_dirty.entry(slot as u32).or_default();
        let mut node_dirty = AuiInvalidation::default();
        for invalidation in &property.invalidates {
            match invalidation {
                Kind::Measure => {
                    custom_dirty.measure = true;
                    custom_dirty.layout = true;
                    custom_dirty.paint = true;
                    node_dirty.measure = true;
                    node_dirty.layout = true;
                    node_dirty.paint = true;
                    node_dirty.hit = true;
                    if let Some(runtime) = state.source_node.custom_runtime.as_mut() {
                        runtime.measure = None;
                        runtime.measure_identity = None;
                        runtime.child_rects.clear();
                        runtime.layout_identity = None;
                        runtime.paint = None;
                        runtime.paint_identity = None;
                    }
                }
                Kind::Layout => {
                    custom_dirty.layout = true;
                    custom_dirty.paint = true;
                    node_dirty.layout = true;
                    node_dirty.paint = true;
                    node_dirty.hit = true;
                    if let Some(runtime) = state.source_node.custom_runtime.as_mut() {
                        runtime.child_rects.clear();
                        runtime.layout_identity = None;
                        runtime.paint = None;
                        runtime.paint_identity = None;
                    }
                }
                Kind::Paint => {
                    custom_dirty.paint = true;
                    node_dirty.paint = true;
                    if let Some(runtime) = state.source_node.custom_runtime.as_mut() {
                        runtime.paint = None;
                        runtime.paint_identity = None;
                    }
                }
                Kind::Hit => node_dirty.hit = true,
            }
        }
        state.dirty.merge(node_dirty);
        if !node_dirty.is_empty() && !state.queued {
            state.queued = true;
            self.dirty.push(slot as u32);
        }
        let document = Arc::make_mut(&mut self.source);
        document.nodes[slot].custom = state.source_node.custom.clone();
        document.nodes[slot].custom_runtime = state.source_node.custom_runtime.clone();
        self.content_revision = self.content_revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        self.record_node_change(slot as u32, AuiPresentationChange::Custom);
        Ok(true)
    }

    pub(crate) fn install_custom_output(
        &mut self,
        handle: UiNodeHandle,
        output: project_runtime_sdk::aui_extension::UiExtensionOutput,
    ) -> Result<(), UiError> {
        self.install_custom_output_with_identity(handle, output, None)
            .map(|_| ())
    }

    pub(crate) fn install_custom_output_with_identity(
        &mut self,
        handle: UiNodeHandle,
        output: project_runtime_sdk::aui_extension::UiExtensionOutput,
        identity: Option<crate::aui_extension::AuiCustomStageIdentity>,
    ) -> Result<bool, UiError> {
        let slot = self.live_slot(handle)?;
        let state = &mut self.nodes[slot];
        let runtime = state
            .source_node
            .custom_runtime
            .get_or_insert_with(Default::default);
        let dirty = self.custom_dirty.entry(slot as u32).or_default();
        let changed = match output {
            project_runtime_sdk::aui_extension::UiExtensionOutput::Describe { .. } => false,
            project_runtime_sdk::aui_extension::UiExtensionOutput::Measure { output } => {
                let changed = runtime.measure.as_ref() != Some(&output);
                runtime.measure = Some(output);
                runtime.measure_identity = identity;
                dirty.measure = false;
                changed
            }
            project_runtime_sdk::aui_extension::UiExtensionOutput::Layout { children } => {
                let changed = runtime.child_rects != children;
                runtime.child_rects = children;
                runtime.layout_identity = identity;
                dirty.layout = false;
                changed
            }
            project_runtime_sdk::aui_extension::UiExtensionOutput::Paint { output } => {
                let changed = runtime.paint.as_ref() != Some(&output);
                runtime.paint = Some(output);
                runtime.paint_identity = identity;
                dirty.paint = false;
                changed
            }
        };
        if !dirty.measure && !dirty.layout && !dirty.paint {
            self.custom_dirty.remove(&(slot as u32));
        }
        if !changed {
            return Ok(false);
        }
        runtime.revision = runtime.revision.saturating_add(1);
        let document = Arc::make_mut(&mut self.source);
        if let Some(node) = document.nodes.get_mut(slot) {
            node.custom_runtime = state.source_node.custom_runtime.clone();
        }
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        self.content_revision = self.content_revision.saturating_add(1);
        self.record_node_change(slot as u32, AuiPresentationChange::Custom);
        Ok(true)
    }

    pub(crate) fn complete_custom_stage(
        &mut self,
        handle: UiNodeHandle,
        stage: project_runtime_sdk::aui_extension::UiExtensionInvalidation,
    ) -> Result<(), UiError> {
        let slot = self.live_slot(handle)? as u32;
        let Some(dirty) = self.custom_dirty.get_mut(&slot) else {
            return Ok(());
        };
        match stage {
            project_runtime_sdk::aui_extension::UiExtensionInvalidation::Measure => {
                dirty.measure = false
            }
            project_runtime_sdk::aui_extension::UiExtensionInvalidation::Layout => {
                dirty.layout = false
            }
            project_runtime_sdk::aui_extension::UiExtensionInvalidation::Paint => {
                dirty.paint = false
            }
            project_runtime_sdk::aui_extension::UiExtensionInvalidation::Hit => {}
        }
        if !dirty.measure && !dirty.layout && !dirty.paint {
            self.custom_dirty.remove(&slot);
        }
        Ok(())
    }

    pub(crate) fn discard_stale_custom_output(
        &mut self,
        handle: UiNodeHandle,
        stage: project_runtime_sdk::aui_extension::UiExtensionInvalidation,
        current_identity: &crate::aui_extension::AuiCustomStageIdentity,
    ) -> Result<bool, UiError> {
        use project_runtime_sdk::aui_extension::UiExtensionInvalidation as Stage;
        let slot = self.live_slot(handle)?;
        let state = &mut self.nodes[slot];
        let runtime = state
            .source_node
            .custom_runtime
            .get_or_insert_with(Default::default);
        let changed = match stage {
            Stage::Measure if runtime.measure_identity.as_ref() != Some(current_identity) => {
                let changed = runtime.measure.take().is_some();
                runtime.measure_identity = None;
                changed
            }
            Stage::Layout if runtime.layout_identity.as_ref() != Some(current_identity) => {
                let changed = !runtime.child_rects.is_empty();
                runtime.child_rects.clear();
                runtime.layout_identity = None;
                changed
            }
            Stage::Paint if runtime.paint_identity.as_ref() != Some(current_identity) => {
                let changed = runtime.paint.take().is_some();
                runtime.paint_identity = None;
                changed
            }
            Stage::Hit | Stage::Measure | Stage::Layout | Stage::Paint => false,
        };
        if !changed {
            return Ok(false);
        }
        runtime.revision = runtime.revision.saturating_add(1);
        let document = Arc::make_mut(&mut self.source);
        document.nodes[slot].custom_runtime = state.source_node.custom_runtime.clone();
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        self.content_revision = self.content_revision.saturating_add(1);
        self.record_node_change(slot as u32, AuiPresentationChange::Custom);
        Ok(true)
    }

    pub fn is_effectively_active(&self, handle: UiNodeHandle) -> Result<bool, UiError> {
        Ok(self.state(handle)?.active_in_hierarchy)
    }

    pub fn subscribe(
        &mut self,
        target: UiNodeHandle,
        filter: AuiEventFilter,
        scope: AuiSubscriptionScope,
        callback_id: u64,
    ) -> Result<AuiSubscriptionToken, UiError> {
        let token = self.reserve_subscription(target)?;
        self.subscribe_reserved(token, target, filter, scope, callback_id)?;
        Ok(token)
    }

    pub(crate) fn reserve_subscription(
        &mut self,
        target: UiNodeHandle,
    ) -> Result<AuiSubscriptionToken, UiError> {
        self.state(target)?;
        let id = self.next_subscription_id;
        self.next_subscription_id = self
            .next_subscription_id
            .checked_add(1)
            .ok_or_else(|| error(UiErrorKind::CapacityExceeded, Some(target), None))?;
        Ok(AuiSubscriptionToken {
            instance: self.identity,
            id,
        })
    }

    pub(crate) fn subscribe_reserved(
        &mut self,
        token: AuiSubscriptionToken,
        target: UiNodeHandle,
        filter: AuiEventFilter,
        scope: AuiSubscriptionScope,
        callback_id: u64,
    ) -> Result<(), UiError> {
        let active = self.state(target)?.active_in_hierarchy;
        if token.instance != self.identity
            || token.id == 0
            || token.id >= self.next_subscription_id
            || self.subscriptions.contains_key(&token.id)
            || callback_id == 0
        {
            return Err(error(UiErrorKind::InvalidValue, Some(target), None));
        }
        if self.subscriptions.len() >= MAX_AUI_SUBSCRIPTIONS {
            return Err(error(UiErrorKind::CapacityExceeded, Some(target), None));
        }
        self.subscriptions.insert(
            token.id,
            AuiSubscription {
                target,
                filter,
                scope,
                callback_id,
            },
        );
        if matches!(filter, AuiEventFilter::All | AuiEventFilter::Lifecycle) {
            self.queue_notification(target, AuiNotificationKind::Mount, Some(token.id));
            if active {
                self.queue_notification(target, AuiNotificationKind::Enable, Some(token.id));
            }
        }
        Ok(())
    }

    /// Idempotent cancellation. A token from another instance is never accepted.
    pub fn cancel_subscription(&mut self, token: AuiSubscriptionToken) -> bool {
        token.instance == self.identity && self.subscriptions.remove(&token.id).is_some()
    }

    pub(crate) fn contains_subscription(&self, token: AuiSubscriptionToken) -> bool {
        token.instance == self.identity && self.subscriptions.contains_key(&token.id)
    }

    pub(crate) fn clear_subscriptions(&mut self) {
        self.subscriptions.clear();
        self.notifications.clear();
        self.dispatch = None;
    }

    pub(crate) fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    pub fn begin_dispatch(&mut self) -> Result<Option<AuiNotification>, AuiDispatchError> {
        if self.dispatch.is_some() {
            return Err(AuiDispatchError::Reentrant);
        }
        let Some(queued) = self.notifications.pop_front() else {
            self.cleanup_retired_subscriptions();
            if self.disposed {
                self.subscriptions.clear();
            }
            return Ok(None);
        };
        let notification = queued.notification.clone();
        self.dispatch = Some(AuiDispatchState {
            queued,
            max_token: self.next_subscription_id.saturating_sub(1),
            cursor: 0,
        });
        Ok(Some(notification))
    }

    pub fn next_dispatch_listener(
        &mut self,
    ) -> Result<Option<AuiDispatchListener>, AuiDispatchError> {
        let (cursor, max_token, only_token, active_at_enqueue, notification) = {
            let dispatch = self
                .dispatch
                .as_ref()
                .ok_or(AuiDispatchError::NotDispatching)?;
            (
                dispatch.cursor,
                dispatch.max_token,
                dispatch.queued.only_token,
                dispatch.queued.active_at_enqueue,
                dispatch.queued.notification.clone(),
            )
        };
        let next = self
            .subscriptions
            .range((Excluded(cursor), Included(max_token)))
            .find_map(|(&id, subscription)| {
                (only_token.is_none_or(|only| only == id)
                    && self.subscription_receives(subscription, &notification, active_at_enqueue))
                .then_some((id, subscription.callback_id))
            });
        let Some((id, callback_id)) = next else {
            return Ok(None);
        };
        self.dispatch
            .as_mut()
            .expect("dispatch remains active")
            .cursor = id;
        Ok(Some(AuiDispatchListener {
            token: AuiSubscriptionToken {
                instance: self.identity,
                id,
            },
            callback_id,
        }))
    }

    pub fn end_dispatch(&mut self) -> Result<(), AuiDispatchError> {
        self.dispatch
            .take()
            .ok_or(AuiDispatchError::NotDispatching)?;
        if self.disposed && self.notifications.is_empty() {
            self.subscriptions.clear();
        } else if self.notifications.is_empty() {
            self.cleanup_retired_subscriptions();
        }
        Ok(())
    }

    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        let lifecycle = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.alive)
            .map(|(slot, node)| (self.handle(slot as u32), node.active_in_hierarchy))
            .collect::<Vec<_>>();
        for &(handle, active) in &lifecycle {
            if active {
                self.queue_notification(handle, AuiNotificationKind::Disable, None);
            }
            self.queue_notification(handle, AuiNotificationKind::Dispose, None);
        }
        self.disposed = true;
        self.published_layout = None;
        self.published_matrices.clear();
        self.committed_geometry.clear();
        self.nodes.clear();
        self.by_id.clear();
        self.dirty.clear();
        self.layouts.clear();
        self.layout_source = None;
        self.presentation_properties.clear();
        self.animation_values.clear();
        self.base_revisions.clear();
        self.presentation_changes.clear();
        if self.notifications.is_empty() && self.dispatch.is_none() {
            self.subscriptions.clear();
        }
    }

    fn queue_notification(
        &mut self,
        target: UiNodeHandle,
        kind: AuiNotificationKind,
        only_token: Option<u64>,
    ) {
        let active_at_enqueue = self
            .nodes
            .get(target.slot as usize)
            .is_some_and(|node| node.active_in_hierarchy);
        let interested = match only_token {
            Some(id) => self.subscriptions.contains_key(&id),
            None => self.subscriptions.values().any(|subscription| {
                subscription.target == target
                    && filter_matches(subscription.filter, &kind)
                    && (subscription.scope == AuiSubscriptionScope::Instance
                        || active_scope_receives(&kind, active_at_enqueue))
            }),
        };
        if !interested {
            return;
        }
        let sequence = self.next_notification_sequence;
        self.next_notification_sequence = self
            .next_notification_sequence
            .checked_add(1)
            .expect("AUI notification sequence exhausted");
        self.notifications.push_back(QueuedNotification {
            notification: AuiNotification {
                sequence,
                target,
                kind,
            },
            only_token,
            active_at_enqueue,
        });
    }

    fn subscription_receives(
        &self,
        subscription: &AuiSubscription,
        notification: &AuiNotification,
        active_at_enqueue: bool,
    ) -> bool {
        if subscription.target != notification.target
            || !filter_matches(subscription.filter, &notification.kind)
        {
            return false;
        }
        if subscription.scope == AuiSubscriptionScope::Instance {
            return true;
        }
        active_scope_receives(&notification.kind, active_at_enqueue)
            && match notification.kind {
                AuiNotificationKind::Changed { .. } => self
                    .nodes
                    .get(notification.target.slot as usize)
                    .is_some_and(|node| node.active_in_hierarchy),
                _ => true,
            }
    }

    fn cleanup_retired_subscriptions(&mut self) {
        if self.retired_handles.is_empty() {
            return;
        }
        self.subscriptions
            .retain(|_, subscription| !self.retired_handles.contains(&subscription.target));
        self.retired_handles.clear();
    }

    fn recompute_active_subtrees(&mut self, changed_slots: &[u32]) -> Vec<(UiNodeHandle, bool)> {
        if changed_slots.is_empty() {
            return Vec::new();
        }
        let changed = changed_slots.iter().copied().collect::<BTreeSet<_>>();
        let roots = changed
            .iter()
            .copied()
            .filter(|slot| {
                let mut parent = self.nodes[*slot as usize].parent;
                while let Some(parent_slot) = parent {
                    if changed.contains(&parent_slot) {
                        return false;
                    }
                    parent = self.nodes[parent_slot as usize].parent;
                }
                true
            })
            .collect::<Vec<_>>();
        let mut transitions = Vec::new();
        for root in roots {
            let parent_active = self.nodes[root as usize]
                .parent
                .map(|parent| self.nodes[parent as usize].active_in_hierarchy)
                .unwrap_or(true);
            self.recompute_active_subtree(root, parent_active, &mut transitions);
        }
        transitions
    }

    fn recompute_active_subtree(
        &mut self,
        slot: u32,
        parent_active: bool,
        transitions: &mut Vec<(UiNodeHandle, bool)>,
    ) {
        if !self.nodes[slot as usize].alive {
            return;
        }
        let (changed, active, children, kind, text, visibility, generation) = {
            let node = &mut self.nodes[slot as usize];
            let active = parent_active && node.active;
            let changed = node.active_in_hierarchy != active;
            node.active_in_hierarchy = active;
            if changed {
                let invalidation = AuiInvalidation {
                    measure: true,
                    layout: true,
                    paint: true,
                    hit: true,
                    ..Default::default()
                };
                node.dirty.merge(invalidation);
                if !node.queued {
                    node.queued = true;
                    self.dirty.push(slot);
                }
            }
            (
                changed,
                active,
                node.children.clone(),
                node.kind,
                (node.kind == UiNodeType::Text).then(|| node.text.clone()),
                node.visibility,
                node.generation,
            )
        };
        if changed {
            for solver in self.layouts.values_mut() {
                solver.update_instance_node(
                    slot as usize,
                    (kind == UiNodeType::Text).then_some(text.as_deref().unwrap_or_default()),
                    effective_layout_visibility(active, visibility),
                );
            }
            transitions.push((
                UiNodeHandle {
                    instance: self.identity,
                    slot,
                    generation,
                },
                active,
            ));
        }
        for child in children {
            self.recompute_active_subtree(child, active, transitions);
        }
    }

    fn handle(&self, slot: u32) -> UiNodeHandle {
        UiNodeHandle {
            instance: self.identity,
            slot,
            generation: self.nodes[slot as usize].generation,
        }
    }

    fn state(&self, handle: UiNodeHandle) -> Result<&NodeState, UiError> {
        if self.disposed {
            return Err(error(UiErrorKind::Disposed, Some(handle), None));
        }
        self.nodes
            .get(handle.slot as usize)
            .filter(|node| {
                node.alive
                    && handle.instance == self.identity
                    && handle.generation == node.generation
            })
            .ok_or_else(|| error(UiErrorKind::InvalidHandle, Some(handle), None))
    }
}

fn next_dynamic_generation() -> Result<u32, UiError> {
    NEXT_DYNAMIC_GENERATION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            (value != u32::MAX).then_some(value + 1)
        })
        .map_err(|_| error(UiErrorKind::CapacityExceeded, None, None))
}

fn node_state(
    source_node: AuiNode,
    generation: u32,
    canvas_root: u32,
    template_instance_id: Option<u64>,
) -> NodeState {
    let binding_owned = binding_ownership(&source_node.binding_refs);
    let visibility = match source_node.layout_visibility {
        Some(LayoutVisibility::Collapsed) => UiVisibility::Collapsed,
        Some(LayoutVisibility::Hidden) => UiVisibility::Hidden,
        _ if !source_node.visible => UiVisibility::Hidden,
        _ => UiVisibility::Visible,
    };
    let kind = match source_node.kind {
        AuiNodeKind::Text => UiNodeType::Text,
        AuiNodeKind::Button => UiNodeType::Button,
        AuiNodeKind::Slider => UiNodeType::Slider,
        _ => UiNodeType::Node,
    };
    let text = source_node.text.clone().unwrap_or_default();
    let value = crate::aui::normalize_slider_value(
        &source_node,
        source_node
            .slider_value
            .unwrap_or(source_node.slider_min.unwrap_or(0.0)),
    );
    let interactable = source_node.interactable;
    let source_presentation = source_node.presentation.unwrap_or_default();
    let resource = graphics_resource(&source_node);
    NodeState {
        source_node,
        alive: true,
        kind,
        generation,
        text,
        value,
        translation: source_presentation.transform.translation,
        presentation: source_presentation,
        resource,
        local_style: UiLocalStyle::default(),
        interactable,
        visibility,
        active: true,
        active_in_hierarchy: true,
        parent: None,
        children: Vec::new(),
        canvas_root,
        template_instance_id,
        binding_owned,
        dirty: AuiInvalidation {
            style: true,
            measure: true,
            layout: true,
            paint: true,
            transform: true,
            hit: true,
        },
        queued: true,
    }
}

fn binding_ownership(bindings: &[crate::aui::AuiBindingRef]) -> [bool; 13] {
    let mut owned = [false; 13];
    for binding in bindings {
        if let Some(property) = binding_target_property(binding.target_field) {
            owned[property_index(property)] = true;
        }
    }
    owned
}

fn graphics_resource(node: &AuiNode) -> Option<UiResourceRef> {
    node.custom_runtime
        .as_ref()?
        .paint
        .as_ref()?
        .primitives
        .iter()
        .find_map(|primitive| match primitive {
            project_runtime_sdk::aui_extension::UiExtensionPaintPrimitive::GraphicsAsset {
                asset_id,
                guid,
            } => Some(UiResourceRef {
                asset_id: asset_id.clone(),
                guid: guid.clone(),
            }),
            _ => None,
        })
}

fn declares_binding(node: &AuiNode, property: UiProperty) -> bool {
    node.binding_refs
        .iter()
        .any(|binding| binding_target_property(binding.target_field) == Some(property))
}

fn binding_target_property(target: AuiBindingTarget) -> Option<UiProperty> {
    match target {
        target if target.presentation_property().is_some() => target.presentation_property(),
        AuiBindingTarget::TextText | AuiBindingTarget::InputFieldText => Some(UiProperty::Text),
        AuiBindingTarget::Interactable => Some(UiProperty::Interactable),
        AuiBindingTarget::SliderValue => Some(UiProperty::Value),
        AuiBindingTarget::PanelVisible | AuiBindingTarget::ImageVisible => {
            Some(UiProperty::Visibility)
        }
        // Rect offsets and visual-only targets do not map to the retained setter surface.
        _ => None,
    }
}

fn insert_child(children: &mut Vec<u32>, child: u32, sibling_index: Option<u32>) {
    let index = sibling_index
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(children.len())
        .min(children.len());
    children.insert(index, child);
}

impl UiRead for AuiInstance {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
        Ok(self.state(handle)?.kind)
    }

    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
        let node = self.state(handle)?;
        if (property == UiProperty::Text && node.kind != UiNodeType::Text)
            || (property == UiProperty::Value && node.kind != UiNodeType::Slider)
        {
            return Err(error(
                UiErrorKind::WrongNodeType,
                Some(handle),
                Some(property),
            ));
        }
        Ok(node_value(node, property))
    }
}

/// Property writes coalesce inside one segment. Notifications are queued only after commit.
pub struct AuiEdit<'a> {
    instance: &'a mut AuiInstance,
    pending: BTreeMap<(u32, UiProperty), (UiValue, bool)>,
    pending_ownership: BTreeMap<(u32, UiProperty), UiPropertyOwner>,
    text_bytes: usize,
    failure: Option<UiError>,
    origin: AuiChangeOrigin,
    default_notify: bool,
}

impl AuiEdit<'_> {
    pub fn pending_properties(&self) -> usize {
        self.pending.len()
    }

    pub fn transfer_property_ownership(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        owner: UiPropertyOwner,
    ) -> Result<(), UiError> {
        let node = self.instance.state(handle)?;
        let fail = |kind| error(kind, Some(handle), Some(property));
        if !declares_binding(&node.source_node, property) {
            return Err(fail(UiErrorKind::BindingUnavailable));
        }
        let key = (handle.slot, property);
        let current = if node.binding_owned[property_index(property)] {
            UiPropertyOwner::Binding
        } else {
            UiPropertyOwner::Controller
        };
        if current == owner {
            self.pending_ownership.remove(&key);
            return Ok(());
        }
        if owner == UiPropertyOwner::Binding && self.pending.contains_key(&key) {
            return Err(fail(UiErrorKind::OwnershipValueRequired));
        }
        if !self.pending_ownership.contains_key(&key)
            && self.pending.len() + self.pending_ownership.len() >= MAX_AUI_EDIT_PROPERTIES
        {
            return Err(fail(UiErrorKind::CapacityExceeded));
        }
        self.pending_ownership.insert(key, owner);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), UiError> {
        for (&(slot, property), &owner) in &self.pending_ownership {
            if owner == UiPropertyOwner::Controller && !self.pending.contains_key(&(slot, property))
            {
                return Err(error(
                    UiErrorKind::OwnershipValueRequired,
                    Some(self.instance.handle(slot)),
                    Some(property),
                ));
            }
        }
        Ok(())
    }

    pub fn commit(self) -> Result<usize, UiError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.validate()?;
        if self.instance.disposed {
            return Err(error(UiErrorKind::Disposed, None, None));
        }
        // Exclusive borrow keeps handles and ownership stable between validation and commit.
        let ownership_changes = self.pending_ownership.len();
        let count = self.pending.len() + ownership_changes;
        if ownership_changes > 0
            || self
                .pending
                .keys()
                .any(|(_, property)| !property.is_presentation())
        {
            self.instance.content_revision = self
                .instance
                .content_revision
                .checked_add(1)
                .expect("UI content revision exhausted");
        }
        if count > 0 {
            self.instance.presentation_revision = self
                .instance
                .presentation_revision
                .checked_add(1)
                .expect("UI presentation revision exhausted");
        }
        let mut changes = Vec::with_capacity(self.pending.len());
        let mut presentation_changes = Vec::with_capacity(count + self.pending.len());
        let mut active_slots = Vec::new();
        for ((slot, property), owner) in self.pending_ownership {
            *self
                .instance
                .base_revisions
                .entry((slot, property))
                .or_default() += 1;
            presentation_changes.push((slot, property));
            let node = &mut self.instance.nodes[slot as usize];
            node.binding_owned[property_index(property)] = owner == UiPropertyOwner::Binding;
            if owner == UiPropertyOwner::Binding {
                self.instance
                    .presentation_properties
                    .remove(&(slot, property));
                let invalidation = ownership_release_invalidation(property, node);
                node.dirty.merge(invalidation);
                if !invalidation.is_empty() && !node.queued {
                    node.queued = true;
                    self.instance.dirty.push(slot);
                }
            }
        }
        for ((slot, property), (value, notify)) in self.pending {
            *self
                .instance
                .base_revisions
                .entry((slot, property))
                .or_default() += 1;
            presentation_changes.push((slot, property));
            if property == UiProperty::Active {
                presentation_changes.push((slot, UiProperty::Visibility));
            }
            self.instance
                .presentation_properties
                .insert((slot, property));
            if property == UiProperty::Active {
                // Active is materialized after the node's retained visibility, so reenabling
                // restores Hidden/Collapsed exactly instead of inventing a second visibility.
                self.instance
                    .presentation_properties
                    .insert((slot, UiProperty::Visibility));
            }
            let node = &mut self.instance.nodes[slot as usize];
            let old_value = node_value(node, property);
            let invalidation = invalidation(property, &value, node);
            match value {
                ref value if property.is_presentation() && property != UiProperty::Translation => {
                    node.presentation.apply(property, value)
                }
                UiValue::Resource(resource) => node.resource = resource,
                UiValue::LocalStyle(style) => node.local_style = style,
                UiValue::Text(text) => node.text = text,
                UiValue::Number(value) => {
                    node.value = crate::aui::normalize_slider_value(&node.source_node, value)
                }
                UiValue::Translation(translation) => node.translation = translation,
                UiValue::Bool(value) if property == UiProperty::Active => {
                    node.active = value;
                    active_slots.push(slot);
                }
                UiValue::Bool(interactable) => node.interactable = interactable,
                UiValue::Visibility(visibility) => node.visibility = visibility,
                _ => unreachable!("validated property value"),
            }
            if matches!(property, UiProperty::Text | UiProperty::Visibility) {
                for solver in self.instance.layouts.values_mut() {
                    solver.update_instance_node(
                        slot as usize,
                        (node.kind == UiNodeType::Text).then_some(node.text.as_str()),
                        effective_layout_visibility(node.active_in_hierarchy, node.visibility),
                    );
                }
            }
            node.dirty.merge(invalidation);
            if !invalidation.is_empty() && !node.queued {
                node.queued = true;
                self.instance.dirty.push(slot);
            }
            if notify {
                changes.push((
                    UiNodeHandle {
                        instance: self.instance.identity,
                        slot,
                        generation: node.generation,
                    },
                    property,
                    old_value,
                    node_value(node, property),
                ));
            }
        }
        for (slot, property) in presentation_changes {
            self.instance.record_presentation_change(slot, property);
        }
        let active_transitions = self.instance.recompute_active_subtrees(&active_slots);
        for (target, property, old_value, new_value) in changes {
            self.instance.queue_notification(
                target,
                AuiNotificationKind::Changed {
                    property,
                    old_value,
                    new_value,
                    origin: self.origin,
                },
                None,
            );
        }
        for (target, active) in active_transitions {
            self.instance.queue_notification(
                target,
                if active {
                    AuiNotificationKind::Enable
                } else {
                    AuiNotificationKind::Disable
                },
                None,
            );
        }
        Ok(count)
    }

    fn stage(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        next: UiValue,
        notify: bool,
    ) -> Result<(), UiError> {
        let node = self.instance.state(handle)?;
        let fail = |kind| error(kind, Some(handle), Some(property));
        let pending_owner = self
            .pending_ownership
            .get(&(handle.slot, property))
            .copied();
        if pending_owner == Some(UiPropertyOwner::Binding)
            || (pending_owner.is_none() && node.binding_owned[property_index(property)])
        {
            return Err(fail(UiErrorKind::BindingOwned));
        }
        match (property, &next) {
            (UiProperty::Text, UiValue::Text(_)) if node.kind != UiNodeType::Text => {
                return Err(fail(UiErrorKind::WrongNodeType));
            }
            (UiProperty::Value, UiValue::Number(_)) if node.kind != UiNodeType::Slider => {
                return Err(fail(UiErrorKind::WrongNodeType));
            }
            (UiProperty::Translation, UiValue::Translation(v))
                if !v.iter().all(|v| v.is_finite()) =>
            {
                return Err(fail(UiErrorKind::InvalidValue));
            }
            (UiProperty::Value, UiValue::Number(value)) if !value.is_finite() => {
                return Err(fail(UiErrorKind::InvalidValue));
            }
            (UiProperty::Text, UiValue::Text(_))
            | (UiProperty::Translation, UiValue::Translation(_))
            | (UiProperty::Interactable, UiValue::Bool(_))
            | (UiProperty::Active, UiValue::Bool(_))
            | (UiProperty::Visibility, UiValue::Visibility(_))
            | (UiProperty::Value, UiValue::Number(_)) => {}
            _ => project_runtime_sdk::aui_wire::validate_value(handle, property, &next)?,
        }
        if property == UiProperty::Resource {
            if !matches!(
                node.source_node.kind,
                AuiNodeKind::Image | AuiNodeKind::Custom
            ) {
                return Err(fail(UiErrorKind::WrongNodeType));
            }
            if let UiValue::Resource(Some(resource)) = &next {
                let valid = if node.source_node.kind == AuiNodeKind::Custom {
                    self.instance
                        .source
                        .graphics_assets
                        .resolve(&resource.asset_id, &resource.guid)
                        .is_ok()
                } else {
                    self.instance
                        .source
                        .resource_identities
                        .get(&resource.asset_id)
                        .is_some_and(|(guid, kind)| guid == &resource.guid && kind == "texture")
                };
                if !valid {
                    return Err(fail(UiErrorKind::InvalidValue));
                }
            }
        }
        let key = (handle.slot, property);
        let old_bytes = self
            .pending
            .get(&key)
            .map_or(0, |(value, _)| text_bytes(value));
        // Compare in place: no cloning the old text just to decide whether it changed.
        if equals(node, property, &next) && pending_owner != Some(UiPropertyOwner::Controller) {
            self.pending.remove(&key);
            self.text_bytes -= old_bytes;
            return Ok(());
        }
        let bytes = self.text_bytes - old_bytes + text_bytes(&next);
        if bytes > MAX_AUI_EDIT_TEXT_BYTES
            || (!self.pending.contains_key(&key)
                && self.pending.len() + self.pending_ownership.len() >= MAX_AUI_EDIT_PROPERTIES)
        {
            return Err(fail(UiErrorKind::CapacityExceeded));
        }
        self.pending.insert(key, (next, notify));
        self.text_bytes = bytes;
        Ok(())
    }
}

fn layout_visibility(visibility: UiVisibility) -> LayoutVisibility {
    match visibility {
        UiVisibility::Visible => LayoutVisibility::Visible,
        UiVisibility::Hidden => LayoutVisibility::Hidden,
        UiVisibility::Collapsed => LayoutVisibility::Collapsed,
    }
}

fn effective_layout_visibility(active: bool, visibility: UiVisibility) -> LayoutVisibility {
    if active {
        layout_visibility(visibility)
    } else {
        LayoutVisibility::Collapsed
    }
}

fn filter_matches(filter: AuiEventFilter, kind: &AuiNotificationKind) -> bool {
    match (filter, kind) {
        (AuiEventFilter::All, _) => true,
        (
            AuiEventFilter::Lifecycle,
            AuiNotificationKind::Mount
            | AuiNotificationKind::Enable
            | AuiNotificationKind::Disable
            | AuiNotificationKind::Dispose,
        ) => true,
        (AuiEventFilter::Changed(expected), AuiNotificationKind::Changed { property, .. }) => {
            expected.is_none_or(|expected| expected == *property)
        }
        _ => false,
    }
}

fn active_scope_receives(kind: &AuiNotificationKind, active_at_enqueue: bool) -> bool {
    match kind {
        AuiNotificationKind::Mount | AuiNotificationKind::Enable | AuiNotificationKind::Disable => {
            true
        }
        AuiNotificationKind::Dispose => false,
        AuiNotificationKind::Changed { .. } | AuiNotificationKind::Animation { .. } => {
            active_at_enqueue
        }
    }
}

impl UiRead for AuiEdit<'_> {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.instance.node_type(handle)
    }

    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.instance.state(handle)?;
        match self.pending.get(&(handle.slot, property)) {
            Some((value, _)) => Ok(value.clone()),
            None => self.instance.read(handle, property),
        }
    }
}

impl UiWrite for AuiEdit<'_> {
    fn write(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if let Err(failure) = self.stage(handle, property, value, self.default_notify) {
            self.pending.clear();
            self.text_bytes = 0;
            self.failure = Some(failure);
            return Err(failure);
        }
        Ok(())
    }

    fn write_without_notify(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if let Err(failure) = self.stage(handle, property, value, false) {
            self.pending.clear();
            self.text_bytes = 0;
            self.failure = Some(failure);
            return Err(failure);
        }
        Ok(())
    }
}

fn error(kind: UiErrorKind, handle: Option<UiNodeHandle>, property: Option<UiProperty>) -> UiError {
    UiError {
        kind,
        handle,
        property,
    }
}

fn property_index(property: UiProperty) -> usize {
    match property {
        UiProperty::Text => 0,
        UiProperty::Translation => 1,
        UiProperty::Interactable => 2,
        UiProperty::Visibility => 3,
        UiProperty::Active => 4,
        UiProperty::Value => 5,
        UiProperty::Rotation => 6,
        UiProperty::Scale => 7,
        UiProperty::Pivot => 8,
        UiProperty::Opacity => 9,
        UiProperty::Tint => 10,
        UiProperty::Resource => 11,
        UiProperty::LocalStyle => 12,
    }
}

fn node_value(node: &NodeState, property: UiProperty) -> UiValue {
    match property {
        UiProperty::Text => UiValue::Text(node.text.clone()),
        UiProperty::Translation => UiValue::Translation(node.translation),
        UiProperty::Interactable => UiValue::Bool(node.interactable),
        UiProperty::Visibility => UiValue::Visibility(node.visibility),
        UiProperty::Active => UiValue::Bool(node.active),
        UiProperty::Value => UiValue::Number(node.value),
        UiProperty::Rotation => UiValue::Number(node.presentation.transform.rotation),
        UiProperty::Scale => UiValue::Vector(node.presentation.transform.scale),
        UiProperty::Pivot => UiValue::Vector(node.presentation.transform.pivot),
        UiProperty::Opacity => UiValue::Number(node.presentation.opacity),
        UiProperty::Tint => UiValue::Color(node.presentation.tint),
        UiProperty::Resource => UiValue::Resource(node.resource.clone()),
        UiProperty::LocalStyle => UiValue::LocalStyle(node.local_style.clone()),
    }
}

fn equals(node: &NodeState, property: UiProperty, value: &UiValue) -> bool {
    match (property, value) {
        (UiProperty::Text, UiValue::Text(text)) => node.text == *text,
        (UiProperty::Translation, UiValue::Translation(translation)) => {
            node.translation == *translation
        }
        (UiProperty::Interactable, UiValue::Bool(value)) => node.interactable == *value,
        (UiProperty::Active, UiValue::Bool(value)) => node.active == *value,
        (UiProperty::Visibility, UiValue::Visibility(visibility)) => node.visibility == *visibility,
        (UiProperty::Value, UiValue::Number(value)) => {
            node.value == crate::aui::normalize_slider_value(&node.source_node, *value)
        }
        _ => node_value(node, property) == *value,
    }
}

fn text_bytes(value: &UiValue) -> usize {
    match value {
        UiValue::Text(text) => text.len(),
        _ => 0,
    }
}

fn invalidation(property: UiProperty, next: &UiValue, node: &NodeState) -> AuiInvalidation {
    match property {
        UiProperty::Text => AuiInvalidation {
            measure: true,
            layout: true,
            paint: true,
            ..Default::default()
        },
        UiProperty::Translation
        | UiProperty::Rotation
        | UiProperty::Scale
        | UiProperty::Pivot
        | UiProperty::Opacity
        | UiProperty::Tint => AuiInvalidation {
            transform: true,
            hit: true,
            ..Default::default()
        },
        UiProperty::Interactable => AuiInvalidation {
            style: true,
            paint: true,
            hit: true,
            ..Default::default()
        },
        UiProperty::Visibility => AuiInvalidation {
            layout: node.visibility == UiVisibility::Collapsed
                || *next == UiValue::Visibility(UiVisibility::Collapsed),
            paint: true,
            hit: true,
            ..Default::default()
        },
        // Effective active changes invalidate the complete affected subtree in one pass.
        UiProperty::Active => AuiInvalidation::default(),
        UiProperty::Value => AuiInvalidation {
            paint: true,
            ..Default::default()
        },
        UiProperty::Resource | UiProperty::LocalStyle => AuiInvalidation {
            paint: true,
            style: true,
            ..Default::default()
        },
    }
}

fn ownership_release_invalidation(property: UiProperty, node: &NodeState) -> AuiInvalidation {
    match property {
        UiProperty::Text => AuiInvalidation {
            measure: true,
            layout: true,
            paint: true,
            ..Default::default()
        },
        UiProperty::Interactable => AuiInvalidation {
            style: true,
            paint: true,
            hit: true,
            ..Default::default()
        },
        UiProperty::Visibility => AuiInvalidation {
            layout: node.visibility == UiVisibility::Collapsed,
            paint: true,
            hit: true,
            ..Default::default()
        },
        UiProperty::Value => AuiInvalidation {
            paint: true,
            ..Default::default()
        },
        UiProperty::Translation
        | UiProperty::Rotation
        | UiProperty::Scale
        | UiProperty::Pivot
        | UiProperty::Opacity
        | UiProperty::Tint => AuiInvalidation {
            transform: true,
            hit: true,
            ..Default::default()
        },
        UiProperty::Resource | UiProperty::LocalStyle => AuiInvalidation {
            paint: true,
            style: true,
            ..Default::default()
        },
        UiProperty::Active => AuiInvalidation::default(),
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::aui::AuiCanvas;
    use crate::resource_memory::{memory_totals, ResourceMemorySample};

    #[test]
    fn instance_memory_observes_committed_values_notifications_and_retained_capacity() {
        for capacity in [8, 64] {
            let source = Arc::new(AuiDocument::new(
                "memory",
                vec![AuiCanvas::screen_overlay("main", 200.0, 100.0, "text")],
                vec![AuiNode::new(
                    "text",
                    AuiNodeKind::Text,
                    AuiRect::fixed_position(0.0, 0.0, 100.0, 20.0),
                )],
            ));
            let mut instance = AuiInstance::mount(source.clone()).unwrap();
            let target = instance.node("text").unwrap();
            instance
                .subscribe(
                    target,
                    UiEventFilter::Changed(None),
                    UiSubscriptionScope::Instance,
                    1,
                )
                .unwrap();
            let mut text = String::with_capacity(capacity * 64);
            text.push_str("committed");
            let mut edit = instance.edit();
            edit.write(target, UiProperty::Text, UiValue::Text(text))
                .unwrap();
            edit.commit().unwrap();
            instance.nodes.reserve(capacity);
            instance.dirty.reserve(capacity);
            instance.nodes[0].children.reserve(capacity);
            instance.notifications.reserve(capacity);
            instance.presentation_changes.reserve(capacity);
            instance.nodes[0].resource = Some(UiResourceRef {
                asset_id: "resource-id".repeat(capacity),
                guid: "resource-guid".repeat(capacity),
            });
            let text_id = format!("cpu-storage:{:x}", instance.nodes[0].text.as_ptr() as usize);
            let mut expected = vec![
                (
                    instance.nodes.as_ptr() as usize,
                    instance.nodes.capacity() * std::mem::size_of::<NodeState>(),
                ),
                (
                    instance.dirty.as_ptr() as usize,
                    instance.dirty.capacity() * std::mem::size_of::<u32>(),
                ),
                (
                    instance.nodes[0].children.as_ptr() as usize,
                    instance.nodes[0].children.capacity() * std::mem::size_of::<u32>(),
                ),
                (
                    instance.nodes[0].text.as_ptr() as usize,
                    instance.nodes[0].text.capacity(),
                ),
            ];
            for key in instance.by_id.keys() {
                expected.push((key.as_ptr() as usize, key.capacity()));
            }
            let resource = instance.nodes[0].resource.as_ref().unwrap();
            for text in [&resource.asset_id, &resource.guid] {
                expected.push((text.as_ptr() as usize, text.capacity()));
            }
            let strong = Arc::strong_count(&source);
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            instance.observe_memory(&mut sample, "other", "resident");
            for (pointer, bytes) in expected {
                assert!(
                    sample.allocations.iter().any(|a| a.allocation_id
                        == format!("cpu-storage:{pointer:x}")
                        && a.bytes == bytes as u64
                        && a.owner == "other"
                        && a.roles.contains("resident")),
                    "missing actual instance backing {pointer:x}"
                );
            }
            let notification = &instance.notifications[0].notification;
            if let UiNotificationKind::Changed {
                new_value: UiValue::Text(text),
                ..
            } = &notification.kind
            {
                let id = format!("cpu-storage:{:x}", text.as_ptr() as usize);
                assert!(sample.allocations.iter().any(|a| a.allocation_id == id
                    && a.bytes == text.capacity() as u64
                    && a.owner == "queues"));
            } else {
                panic!("actual committed change must retain its text notification");
            }
            let total = memory_totals(&sample.allocations).unwrap().cpu_allocated;
            instance.observe_memory(&mut sample, "other", "resident");
            assert_eq!(
                memory_totals(&sample.allocations).unwrap().cpu_allocated,
                total
            );
            assert_eq!(Arc::strong_count(&source), strong);
            assert_eq!(instance.nodes[0].text, "committed");
            let draft = instance.transaction_draft();
            assert_ne!(draft.nodes.as_ptr(), instance.nodes.as_ptr());
            let mut draft_sample = ResourceMemorySample::new(2, 1, 0);
            draft.observe_memory(&mut draft_sample, "queues", "queuePayload");
            assert!(draft_sample.allocations.iter().any(|a| a.allocation_id
                == format!("cpu-storage:{:x}", draft.nodes.as_ptr() as usize)
                && a.owner == "queues"
                && a.roles.contains("queuePayload")));
            let draft_total = memory_totals(&draft_sample.allocations)
                .unwrap()
                .cpu_allocated;
            draft.observe_memory(&mut sample, "queues", "queuePayload");
            assert_eq!(
                memory_totals(&sample.allocations).unwrap().cpu_allocated,
                total + draft_total,
                "actual transaction draft has distinct runtime backing"
            );
            drop(draft);
            let notification_capacity = instance.notifications.capacity();
            instance.begin_dispatch().unwrap().unwrap();
            let mut dispatch = ResourceMemorySample::new(2, 1, 0);
            instance.observe_memory(&mut dispatch, "other", "resident");
            assert_eq!(
                memory_totals(&dispatch.allocations).unwrap().cpu_allocated,
                total
            );
            assert_eq!(instance.notifications.capacity(), notification_capacity);
            instance.dispose();
            let mut disposed = ResourceMemorySample::new(3, 1, 0);
            instance.observe_memory(&mut disposed, "other", "resident");
            assert!(!disposed
                .allocations
                .iter()
                .any(|a| a.allocation_id == text_id));
            assert!(disposed.allocations.iter().any(|a| a.allocation_id
                == format!("cpu-storage:{:x}", instance.nodes.as_ptr() as usize)
                && a.bytes
                    == (instance.nodes.capacity() * std::mem::size_of::<NodeState>()) as u64));
            assert!(memory_totals(&disposed.allocations).unwrap().cpu_allocated < total);
        }
    }
}
