use crate::aui::{
    AuiComputedRect, AuiDocument, AuiInteractionState, AuiLayoutResult, AuiNode, AuiNodeKind,
    AuiRect, AuiRecycledNodeCleanup, AuiScrollState,
};
use crate::aui_layout_schema::{Dimension, LayoutVisibility, Participation, SlotSpec};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const AUI_VIRTUAL_LIST_SCHEMA_VERSION: &str = "aui-virtual-list.v1";
pub const AUI_VIRTUAL_LIST_ACTION_PAYLOAD_SCHEMA_VERSION: &str =
    "aui-virtual-list-action-payload.v1";
pub const AUI_VIRTUAL_LIST_DEFAULT_MAX_LIVE_INSTANCES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiVirtualListItemContext {
    pub list_node_id: String,
    pub item_key: String,
    pub item_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiVirtualListActionPayload {
    pub schema_version: String,
    pub list_node_id: String,
    pub item_key: String,
    pub item_index: usize,
}

impl From<&AuiVirtualListItemContext> for AuiVirtualListActionPayload {
    fn from(item: &AuiVirtualListItemContext) -> Self {
        Self {
            schema_version: AUI_VIRTUAL_LIST_ACTION_PAYLOAD_SCHEMA_VERSION.to_string(),
            list_node_id: item.list_node_id.clone(),
            item_key: item.item_key.clone(),
            item_index: item.item_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum AuiVirtualListExtentMode {
    Fixed { item_extent: f32 },
    Variable { estimated_item_extent: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListSpec {
    pub schema_version: String,
    pub list_node_id: String,
    pub viewport_extent: f32,
    pub overscan: f32,
    pub max_live_instances: usize,
    pub extent_mode: AuiVirtualListExtentMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiVirtualListDeclaration {
    pub list_node_id: String,
    pub data_path: String,
    pub item_template_node_id: String,
    pub overscan: f32,
    pub max_live_instances: usize,
    pub extent_mode: AuiVirtualListExtentMode,
}

impl AuiVirtualListSpec {
    pub fn fixed(list_node_id: impl Into<String>, viewport_extent: f32, item_extent: f32) -> Self {
        Self {
            schema_version: AUI_VIRTUAL_LIST_SCHEMA_VERSION.to_string(),
            list_node_id: list_node_id.into(),
            viewport_extent,
            overscan: viewport_extent.max(0.0),
            max_live_instances: AUI_VIRTUAL_LIST_DEFAULT_MAX_LIVE_INSTANCES,
            extent_mode: AuiVirtualListExtentMode::Fixed { item_extent },
        }
    }

    pub fn variable(
        list_node_id: impl Into<String>,
        viewport_extent: f32,
        estimated_item_extent: f32,
    ) -> Self {
        Self {
            schema_version: AUI_VIRTUAL_LIST_SCHEMA_VERSION.to_string(),
            list_node_id: list_node_id.into(),
            viewport_extent,
            overscan: viewport_extent.max(0.0),
            max_live_instances: AUI_VIRTUAL_LIST_DEFAULT_MAX_LIVE_INSTANCES,
            extent_mode: AuiVirtualListExtentMode::Variable {
                estimated_item_extent,
            },
        }
    }

    pub fn with_overscan(mut self, overscan: f32) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn with_max_live_instances(mut self, max_live_instances: usize) -> Self {
        self.max_live_instances = max_live_instances;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListItem {
    pub item_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_extent: Option<f32>,
}

impl AuiVirtualListItem {
    pub fn new(item_key: impl Into<String>, measured_extent: Option<f32>) -> Self {
        Self {
            item_key: item_key.into(),
            text: None,
            measured_extent,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListSnapshot {
    pub items: Vec<AuiVirtualListItem>,
}

impl AuiVirtualListSnapshot {
    pub fn new(items: Vec<AuiVirtualListItem>) -> Self {
        Self { items }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListState {
    pub scroll_offset: f32,
    pub focused_key: Option<String>,
    #[serde(default)]
    pub selected_key: Option<String>,
    #[serde(default)]
    extent_by_key: BTreeMap<String, f32>,
    #[serde(default)]
    slot_bindings: BTreeMap<usize, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuiVirtualListAlignment {
    Nearest,
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuiVirtualListNavigation {
    Previous,
    Next,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListAnchor {
    pub item_key: String,
    pub local_offset: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListInstance {
    pub slot_index: usize,
    pub node_id: String,
    pub item_index: usize,
    pub item_key: String,
    pub top: f32,
    pub extent: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListReconciliation {
    pub rebound_instance_count: usize,
    pub retired_instance_count: usize,
    pub recycled_node_ids: Vec<String>,
    pub cleared_hover_count: usize,
    pub cleared_pointer_capture_count: usize,
    pub cleared_focus_count: usize,
    pub cleared_text_edit_count: usize,
    pub cleared_modal_count: usize,
}

impl AuiVirtualListReconciliation {
    fn from_cleanup(
        rebound_instance_count: usize,
        retired_instance_count: usize,
        recycled_node_ids: Vec<String>,
        cleanup: AuiRecycledNodeCleanup,
    ) -> Self {
        Self {
            rebound_instance_count,
            retired_instance_count,
            recycled_node_ids,
            cleared_hover_count: cleanup.cleared_hover_count,
            cleared_pointer_capture_count: cleanup.cleared_pointer_capture_count,
            cleared_focus_count: cleanup.cleared_focus_count,
            cleared_text_edit_count: cleanup.cleared_text_edit_count,
            cleared_modal_count: cleanup.cleared_modal_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListWindow {
    pub schema_version: String,
    pub list_node_id: String,
    pub total_item_count: usize,
    pub total_extent: f32,
    pub scroll_offset: f32,
    pub max_scroll_offset: f32,
    pub first_materialized_index: Option<usize>,
    pub last_materialized_index: Option<usize>,
    pub instances: Vec<AuiVirtualListInstance>,
    pub anchor: Option<AuiVirtualListAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_key: Option<String>,
    pub focused_instance_node_id: Option<String>,
    pub selected_key: Option<String>,
    pub requested_key: Option<String>,
    pub capacity_limited: bool,
    pub reconciliation: AuiVirtualListReconciliation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiVirtualListDiagnostic {
    pub code: String,
    pub item_key: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiVirtualListError {
    pub diagnostics: Vec<AuiVirtualListDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListPresentState {
    lists: BTreeMap<String, AuiVirtualListState>,
    windows: BTreeMap<String, AuiVirtualListWindow>,
    slot_node_ids: BTreeMap<String, BTreeMap<usize, HashSet<String>>>,
    #[serde(skip)]
    lifecycle_stats: AuiVirtualListLifecycleStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuiVirtualListLifecycleStats {
    pub materialized_instance_count: u64,
    pub recycled_instance_count: u64,
    pub disposed_instance_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiVirtualListMaterialization {
    pub document: AuiDocument,
    pub windows: Vec<AuiVirtualListWindow>,
}

impl AuiVirtualListPresentState {
    pub fn lifecycle_stats(&self) -> AuiVirtualListLifecycleStats {
        self.lifecycle_stats
    }

    pub fn apply_actions(
        &mut self,
        actions: &[crate::aui::AuiAction],
        windows: &mut [AuiVirtualListWindow],
    ) -> bool {
        let mut changed = false;
        for action in actions {
            let Some(payload) = action.payload.as_deref().and_then(|payload| {
                serde_json::from_str::<AuiVirtualListActionPayload>(payload).ok()
            }) else {
                continue;
            };
            if payload.schema_version != AUI_VIRTUAL_LIST_ACTION_PAYLOAD_SCHEMA_VERSION {
                continue;
            }
            let Some(state) = self.lists.get_mut(&payload.list_node_id) else {
                continue;
            };
            let Some(window) = windows
                .iter_mut()
                .find(|window| window.list_node_id == payload.list_node_id)
            else {
                continue;
            };
            if !window.instances.iter().any(|instance| {
                instance.item_key == payload.item_key && instance.item_index == payload.item_index
            }) {
                continue;
            }
            if state.selected_key.as_deref() != Some(payload.item_key.as_str()) {
                state.selected_key = Some(payload.item_key.clone());
                changed = true;
            }
            window.selected_key = Some(payload.item_key);
        }
        changed
    }

    pub fn interaction_identity(
        document: &AuiDocument,
        interaction: &AuiInteractionState,
    ) -> Option<String> {
        (!document.virtual_lists.is_empty()).then(|| {
            document
                .virtual_lists
                .iter()
                .map(|declaration| {
                    let scroll = interaction
                        .scroll_offsets
                        .get(&declaration.list_node_id)
                        .map_or(0_u32, |state| state.offset_y.to_bits());
                    format!(
                        "{}:{scroll}:{}",
                        declaration.list_node_id,
                        interaction
                            .focus
                            .focused_node
                            .as_deref()
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("|")
        })
    }

    pub fn materialize(
        &mut self,
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        snapshots: &HashMap<String, AuiVirtualListSnapshot>,
        interaction: &mut AuiInteractionState,
    ) -> Result<AuiVirtualListMaterialization, AuiVirtualListError> {
        let mut materialized = document.clone();
        let source_nodes = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let computed = layout
            .computed_nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node.rect))
            .collect::<HashMap<_, _>>();
        let mut windows = Vec::with_capacity(document.virtual_lists.len());
        let mut appended_nodes = Vec::new();

        for declaration in &document.virtual_lists {
            let Some(snapshot) = snapshots.get(&declaration.data_path) else {
                return Err(single_error(
                    "aui.virtual_list.snapshot_missing",
                    None,
                    format!(
                        "Virtual list '{}' has no snapshot at '{}'.",
                        declaration.list_node_id, declaration.data_path
                    ),
                ));
            };
            let Some(list_rect) = computed.get(declaration.list_node_id.as_str()).copied() else {
                return Err(single_error(
                    "aui.virtual_list.layout_missing",
                    None,
                    format!(
                        "Virtual list '{}' has no committed layout geometry.",
                        declaration.list_node_id
                    ),
                ));
            };
            let Some(template) = source_nodes
                .get(declaration.item_template_node_id.as_str())
                .copied()
            else {
                return Err(single_error(
                    "aui.virtual_list.template_missing",
                    None,
                    format!(
                        "Virtual list '{}' item template is missing.",
                        declaration.list_node_id
                    ),
                ));
            };

            let state = self
                .lists
                .entry(declaration.list_node_id.clone())
                .or_default();
            if let Some(scroll) = interaction.scroll_offsets.get(&declaration.list_node_id) {
                state.scroll_offset = scroll.offset_y;
            }
            if let Some(focused_node) = interaction.focus.focused_node.as_deref() {
                if let Some(previous) = self.windows.get(&declaration.list_node_id) {
                    for instance in &previous.instances {
                        let contains_focus = self
                            .slot_node_ids
                            .get(&declaration.list_node_id)
                            .and_then(|slots| slots.get(&instance.slot_index))
                            .is_some_and(|nodes| nodes.contains(focused_node));
                        if contains_focus {
                            state.focused_key = Some(instance.item_key.clone());
                            break;
                        }
                    }
                }
            }

            let spec = AuiVirtualListSpec {
                schema_version: AUI_VIRTUAL_LIST_SCHEMA_VERSION.to_string(),
                list_node_id: declaration.list_node_id.clone(),
                viewport_extent: list_rect.height,
                overscan: declaration.overscan,
                max_live_instances: declaration.max_live_instances,
                extent_mode: declaration.extent_mode,
            };
            let mut window = AuiVirtualListEngine::update(&spec, snapshot, state, interaction)?;
            let previous_slot_nodes = self
                .slot_node_ids
                .remove(&declaration.list_node_id)
                .unwrap_or_default();
            let rebound_instance_count = window.reconciliation.rebound_instance_count as u64;
            self.lifecycle_stats.materialized_instance_count = self
                .lifecycle_stats
                .materialized_instance_count
                .saturating_add(
                    window
                        .instances
                        .iter()
                        .filter(|instance| !previous_slot_nodes.contains_key(&instance.slot_index))
                        .count() as u64,
                );
            self.lifecycle_stats.materialized_instance_count = self
                .lifecycle_stats
                .materialized_instance_count
                .saturating_add(rebound_instance_count);
            self.lifecycle_stats.recycled_instance_count = self
                .lifecycle_stats
                .recycled_instance_count
                .saturating_add(rebound_instance_count);
            self.lifecycle_stats.disposed_instance_count =
                self.lifecycle_stats.disposed_instance_count.saturating_add(
                    (window.reconciliation.retired_instance_count as u64)
                        .saturating_add(rebound_instance_count),
                );
            let recycled_slots = window
                .reconciliation
                .recycled_node_ids
                .iter()
                .filter_map(|node_id| virtual_slot(&declaration.list_node_id, node_id))
                .collect::<HashSet<_>>();
            let recycled_descendants = recycled_slots
                .iter()
                .flat_map(|slot| previous_slot_nodes.get(slot).into_iter().flatten().cloned())
                .collect::<HashSet<_>>();
            if !recycled_descendants.is_empty() {
                let cleanup = interaction.reconcile_recycled_nodes(&recycled_descendants);
                window.reconciliation.cleared_hover_count += cleanup.cleared_hover_count;
                window.reconciliation.cleared_pointer_capture_count +=
                    cleanup.cleared_pointer_capture_count;
                window.reconciliation.cleared_focus_count += cleanup.cleared_focus_count;
                window.reconciliation.cleared_text_edit_count += cleanup.cleared_text_edit_count;
                window.reconciliation.cleared_modal_count += cleanup.cleared_modal_count;
            }

            let mut slot_nodes = BTreeMap::new();
            for instance in &window.instances {
                let item = &snapshot.items[instance.item_index];
                let clones = clone_template_subtree(
                    template,
                    &source_nodes,
                    &declaration.list_node_id,
                    instance,
                    item,
                    window.scroll_offset,
                    list_rect,
                );
                slot_nodes.insert(
                    instance.slot_index,
                    clones
                        .iter()
                        .map(|node| node.node_id.clone())
                        .collect::<HashSet<_>>(),
                );
                appended_nodes.extend(clones);
            }
            let spacer_id = format!("{}:virtual-extent", declaration.list_node_id);
            appended_nodes.push(virtual_extent_node(
                &spacer_id,
                &declaration.list_node_id,
                list_rect,
                window.total_extent,
            ));
            if let Some(list) = materialized
                .nodes
                .iter_mut()
                .find(|node| node.node_id == declaration.list_node_id)
            {
                list.children.push(spacer_id);
                list.children.extend(
                    window
                        .instances
                        .iter()
                        .map(|instance| instance.node_id.clone()),
                );
            }
            interaction.scroll_offsets.insert(
                declaration.list_node_id.clone(),
                AuiScrollState {
                    node_id: declaration.list_node_id.clone(),
                    offset_y: window.scroll_offset,
                    max_offset_y: window.max_scroll_offset,
                    last_delta_y: 0.0,
                },
            );
            self.slot_node_ids
                .insert(declaration.list_node_id.clone(), slot_nodes);
            self.windows
                .insert(declaration.list_node_id.clone(), window.clone());
            windows.push(window);
        }
        materialized.nodes.extend(appended_nodes);
        Ok(AuiVirtualListMaterialization {
            document: materialized,
            windows,
        })
    }
}

#[derive(Debug, Clone)]
struct PreparedList {
    extents: Vec<f32>,
    tops: Vec<f32>,
    total_extent: f32,
    key_to_index: HashMap<String, usize>,
}

pub struct AuiVirtualListEngine;

impl AuiVirtualListEngine {
    pub fn update(
        spec: &AuiVirtualListSpec,
        snapshot: &AuiVirtualListSnapshot,
        state: &mut AuiVirtualListState,
        interaction: &mut AuiInteractionState,
    ) -> Result<AuiVirtualListWindow, AuiVirtualListError> {
        let prepared = prepare_list(spec, snapshot, state)?;
        let old_anchor = anchor_for_offset(spec, snapshot, state, state.scroll_offset);
        let mut next_state = state.clone();
        if let Some(anchor) = &old_anchor {
            if let Some(index) = prepared.key_to_index.get(&anchor.item_key).copied() {
                next_state.scroll_offset = prepared.tops[index] + anchor.local_offset;
            }
        }
        let max_scroll_offset = (prepared.total_extent - spec.viewport_extent.max(0.0)).max(0.0);
        next_state.scroll_offset = next_state.scroll_offset.clamp(0.0, max_scroll_offset);
        let window = materialize_window(spec, snapshot, prepared, next_state, interaction, None);
        *state = window.1;
        Ok(window.0)
    }

    pub fn scroll_to_item(
        spec: &AuiVirtualListSpec,
        snapshot: &AuiVirtualListSnapshot,
        state: &mut AuiVirtualListState,
        interaction: &mut AuiInteractionState,
        item_key: &str,
        alignment: AuiVirtualListAlignment,
    ) -> Result<AuiVirtualListWindow, AuiVirtualListError> {
        let prepared = prepare_list(spec, snapshot, state)?;
        let Some(index) = prepared.key_to_index.get(item_key).copied() else {
            return Err(single_error(
                "aui.virtual_list.item_key_missing",
                Some(item_key),
                format!("Virtual list item key '{item_key}' does not exist."),
            ));
        };
        let top = prepared.tops[index];
        let bottom = top + prepared.extents[index];
        let viewport = spec.viewport_extent;
        let current = state.scroll_offset;
        let requested = match alignment {
            AuiVirtualListAlignment::Start => top,
            AuiVirtualListAlignment::Center => top - (viewport - prepared.extents[index]) * 0.5,
            AuiVirtualListAlignment::End => bottom - viewport,
            AuiVirtualListAlignment::Nearest if top < current => top,
            AuiVirtualListAlignment::Nearest if bottom > current + viewport => bottom - viewport,
            AuiVirtualListAlignment::Nearest => current,
        };
        let max_scroll_offset = (prepared.total_extent - viewport).max(0.0);
        let mut next_state = state.clone();
        next_state.scroll_offset = requested.clamp(0.0, max_scroll_offset);
        let (window, next_state) = materialize_window(
            spec,
            snapshot,
            prepared,
            next_state,
            interaction,
            Some(item_key.to_string()),
        );
        *state = next_state;
        Ok(window)
    }

    pub fn select_item(
        spec: &AuiVirtualListSpec,
        snapshot: &AuiVirtualListSnapshot,
        state: &mut AuiVirtualListState,
        interaction: &mut AuiInteractionState,
        item_key: &str,
    ) -> Result<AuiVirtualListWindow, AuiVirtualListError> {
        let prepared = prepare_list(spec, snapshot, state)?;
        if !prepared.key_to_index.contains_key(item_key) {
            return Err(single_error(
                "aui.virtual_list.item_key_missing",
                Some(item_key),
                format!("Virtual list item key '{item_key}' does not exist."),
            ));
        }
        let mut next_state = state.clone();
        next_state.selected_key = Some(item_key.to_string());
        let (window, next_state) = materialize_window(
            spec,
            snapshot,
            prepared,
            next_state,
            interaction,
            Some(item_key.to_string()),
        );
        *state = next_state;
        Ok(window)
    }

    pub fn navigate(
        spec: &AuiVirtualListSpec,
        snapshot: &AuiVirtualListSnapshot,
        state: &mut AuiVirtualListState,
        interaction: &mut AuiInteractionState,
        direction: AuiVirtualListNavigation,
    ) -> Result<AuiVirtualListWindow, AuiVirtualListError> {
        let prepared = prepare_list(spec, snapshot, state)?;
        if snapshot.items.is_empty() {
            let mut next_state = state.clone();
            next_state.focused_key = None;
            let (window, next_state) =
                materialize_window(spec, snapshot, prepared, next_state, interaction, None);
            *state = next_state;
            return Ok(window);
        }
        let current = state
            .focused_key
            .as_ref()
            .and_then(|key| prepared.key_to_index.get(key).copied());
        let next_index = match (current, direction) {
            (Some(index), AuiVirtualListNavigation::Next) => {
                (index + 1).min(snapshot.items.len() - 1)
            }
            (Some(index), AuiVirtualListNavigation::Previous) => index.saturating_sub(1),
            (None, AuiVirtualListNavigation::Next) => 0,
            (None, AuiVirtualListNavigation::Previous) => snapshot.items.len() - 1,
        };
        let next_key = snapshot.items[next_index].item_key.clone();
        let mut next_state = state.clone();
        next_state.focused_key = Some(next_key.clone());
        let top = prepared.tops[next_index];
        let bottom = top + prepared.extents[next_index];
        if top < next_state.scroll_offset {
            next_state.scroll_offset = top;
        } else if bottom > next_state.scroll_offset + spec.viewport_extent {
            next_state.scroll_offset = bottom - spec.viewport_extent;
        }
        let max_scroll_offset = (prepared.total_extent - spec.viewport_extent).max(0.0);
        next_state.scroll_offset = next_state.scroll_offset.clamp(0.0, max_scroll_offset);
        let (window, next_state) = materialize_window(
            spec,
            snapshot,
            prepared,
            next_state,
            interaction,
            Some(next_key),
        );
        *state = next_state;
        Ok(window)
    }
}

fn clone_template_subtree(
    template: &AuiNode,
    source_nodes: &HashMap<&str, &AuiNode>,
    list_node_id: &str,
    instance: &AuiVirtualListInstance,
    item: &AuiVirtualListItem,
    scroll_offset: f32,
    list_rect: AuiComputedRect,
) -> Vec<AuiNode> {
    let mut source_ids = Vec::new();
    let mut pending = vec![template.node_id.as_str()];
    while let Some(node_id) = pending.pop() {
        let Some(node) = source_nodes.get(node_id).copied() else {
            continue;
        };
        source_ids.push(node_id);
        pending.extend(node.children.iter().rev().map(String::as_str));
    }
    let remapped = source_ids
        .iter()
        .map(|source_id| {
            let target = if *source_id == template.node_id {
                instance.node_id.clone()
            } else {
                format!("{}:{source_id}", instance.node_id)
            };
            (*source_id, target)
        })
        .collect::<HashMap<_, _>>();
    let mut clones = source_ids
        .iter()
        .filter_map(|source_id| source_nodes.get(source_id).copied())
        .map(|source| {
            let mut node = source.clone();
            node.node_id = remapped[source.node_id.as_str()].clone();
            node.name = format!("{} [{}]", source.name, item.item_key);
            node.parent = if source.node_id == template.node_id {
                Some(list_node_id.to_string())
            } else {
                source
                    .parent
                    .as_deref()
                    .and_then(|parent| remapped.get(parent).cloned())
            };
            node.children = source
                .children
                .iter()
                .filter_map(|child| remapped.get(child.as_str()).cloned())
                .collect();
            if source.node_id == template.node_id {
                node.virtual_list_item = Some(AuiVirtualListItemContext {
                    list_node_id: list_node_id.to_string(),
                    item_key: item.item_key.clone(),
                    item_index: instance.item_index,
                });
                node.layout_visibility = Some(LayoutVisibility::Visible);
                node.resolved_viewport_original_visibility = None;
                node.visible = true;
                node.rect = AuiRect::fixed_position(
                    0.0,
                    instance.top - scroll_offset,
                    list_rect.width,
                    instance.extent,
                );
                node.slot = Some(SlotSpec {
                    width: Dimension::Fixed {
                        value: list_rect.width,
                    },
                    height: Dimension::Fixed {
                        value: instance.extent,
                    },
                    min_height: instance.extent,
                    flex_shrink: 0.0,
                    participation: Participation::OutOfFlow,
                    ..Default::default()
                });
            }
            node
        })
        .collect::<Vec<_>>();
    if let Some(text) = &item.text {
        if let Some(node) = clones
            .iter_mut()
            .find(|node| matches!(node.kind, AuiNodeKind::Text | AuiNodeKind::InputField))
        {
            node.text = Some(text.clone());
            node.binding_refs.clear();
        }
    }
    clones
}

fn virtual_extent_node(
    node_id: &str,
    list_node_id: &str,
    list_rect: AuiComputedRect,
    total_extent: f32,
) -> AuiNode {
    let mut node = AuiNode::new(
        node_id,
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, total_extent, list_rect.width, 0.0),
    )
    .with_parent(list_node_id);
    node.visible = false;
    node.slot = Some(SlotSpec {
        width: Dimension::Fixed {
            value: list_rect.width,
        },
        height: Dimension::Fixed { value: 0.0 },
        flex_shrink: 0.0,
        participation: Participation::OutOfFlow,
        ..Default::default()
    });
    node
}

fn virtual_slot(list_node_id: &str, node_id: &str) -> Option<usize> {
    node_id
        .strip_prefix(&format!("{list_node_id}:virtual-item:"))?
        .parse()
        .ok()
}

fn prepare_list(
    spec: &AuiVirtualListSpec,
    snapshot: &AuiVirtualListSnapshot,
    state: &AuiVirtualListState,
) -> Result<PreparedList, AuiVirtualListError> {
    let mut diagnostics = Vec::new();
    if spec.schema_version != AUI_VIRTUAL_LIST_SCHEMA_VERSION {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.unsupported_schema",
            None,
            format!("Unsupported virtual list schema '{}'.", spec.schema_version),
        );
    }
    if spec.list_node_id.trim().is_empty() {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.list_node_id_required",
            None,
            "Virtual list node id is required.",
        );
    }
    if !valid_positive(spec.viewport_extent) {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.invalid_viewport_extent",
            None,
            "Virtual list viewport extent must be finite and positive.",
        );
    }
    if !spec.overscan.is_finite() || spec.overscan < 0.0 {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.invalid_overscan",
            None,
            "Virtual list overscan must be finite and non-negative.",
        );
    }
    if spec.max_live_instances == 0 || spec.max_live_instances > 4096 {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.invalid_live_instance_limit",
            None,
            "Virtual list max_live_instances must be within 1..=4096.",
        );
    }
    let fallback_extent = match spec.extent_mode {
        AuiVirtualListExtentMode::Fixed { item_extent } => item_extent,
        AuiVirtualListExtentMode::Variable {
            estimated_item_extent,
        } => estimated_item_extent,
    };
    if !valid_positive(fallback_extent) {
        push_diagnostic(
            &mut diagnostics,
            "aui.virtual_list.invalid_default_extent",
            None,
            "Virtual list fixed/default item extent must be finite and positive.",
        );
    }

    let mut seen = HashSet::new();
    let mut extents = Vec::with_capacity(snapshot.items.len());
    let mut tops = Vec::with_capacity(snapshot.items.len());
    let mut key_to_index = HashMap::with_capacity(snapshot.items.len());
    let mut cursor = 0.0_f32;
    for (index, item) in snapshot.items.iter().enumerate() {
        if item.item_key.trim().is_empty() {
            push_diagnostic(
                &mut diagnostics,
                "aui.virtual_list.item_key_required",
                Some(&item.item_key),
                "Virtual list item key is required.",
            );
        } else if !seen.insert(item.item_key.as_str()) {
            push_diagnostic(
                &mut diagnostics,
                "aui.virtual_list.duplicate_key",
                Some(&item.item_key),
                format!("Virtual list item key '{}' is duplicated.", item.item_key),
            );
        }
        if item
            .measured_extent
            .is_some_and(|extent| !valid_positive(extent))
        {
            push_diagnostic(
                &mut diagnostics,
                "aui.virtual_list.invalid_extent",
                Some(&item.item_key),
                format!(
                    "Virtual list item '{}' has an invalid extent.",
                    item.item_key
                ),
            );
        }
        let extent = match spec.extent_mode {
            AuiVirtualListExtentMode::Fixed { item_extent } => item_extent,
            AuiVirtualListExtentMode::Variable {
                estimated_item_extent,
            } => item
                .measured_extent
                .or_else(|| state.extent_by_key.get(&item.item_key).copied())
                .unwrap_or(estimated_item_extent),
        };
        tops.push(cursor);
        extents.push(extent);
        cursor += extent;
        key_to_index.insert(item.item_key.clone(), index);
    }
    if !diagnostics.is_empty() {
        return Err(AuiVirtualListError { diagnostics });
    }
    Ok(PreparedList {
        extents,
        tops,
        total_extent: cursor,
        key_to_index,
    })
}

fn materialize_window(
    spec: &AuiVirtualListSpec,
    snapshot: &AuiVirtualListSnapshot,
    prepared: PreparedList,
    mut next_state: AuiVirtualListState,
    interaction: &mut AuiInteractionState,
    requested_key: Option<String>,
) -> (AuiVirtualListWindow, AuiVirtualListState) {
    if next_state
        .selected_key
        .as_ref()
        .is_some_and(|key| !prepared.key_to_index.contains_key(key))
    {
        next_state.selected_key = None;
    }
    if next_state
        .focused_key
        .as_ref()
        .is_some_and(|key| !prepared.key_to_index.contains_key(key))
    {
        next_state.focused_key = None;
    }
    let range_start = (next_state.scroll_offset - spec.overscan).max(0.0);
    let range_end = (next_state.scroll_offset + spec.viewport_extent + spec.overscan)
        .min(prepared.total_extent);
    let mut visible_indices = Vec::new();
    let mut capacity_limited = false;
    for index in 0..snapshot.items.len() {
        let top = prepared.tops[index];
        let bottom = top + prepared.extents[index];
        if bottom > range_start && top < range_end {
            if visible_indices.len() == spec.max_live_instances {
                capacity_limited = true;
                break;
            }
            visible_indices.push(index);
        }
    }

    let visible_keys = visible_indices
        .iter()
        .map(|index| snapshot.items[*index].item_key.as_str())
        .collect::<HashSet<_>>();
    let old_bindings = next_state.slot_bindings.clone();
    let mut new_bindings = BTreeMap::new();
    for (slot, key) in &old_bindings {
        if visible_keys.contains(key.as_str()) {
            new_bindings.insert(*slot, key.clone());
        }
    }
    let retained_keys = new_bindings.values().cloned().collect::<HashSet<_>>();
    let free_slots = (0..spec.max_live_instances)
        .filter(|slot| !new_bindings.contains_key(slot))
        .collect::<Vec<_>>();
    let mut free_slots = free_slots.into_iter();
    for index in &visible_indices {
        let key = &snapshot.items[*index].item_key;
        if !retained_keys.contains(key) {
            if let Some(slot) = free_slots.next() {
                new_bindings.insert(slot, key.clone());
            }
        }
    }

    let mut recycled_node_ids = Vec::new();
    let mut rebound_instance_count = 0;
    let mut retired_instance_count = 0;
    for (slot, old_key) in &old_bindings {
        match new_bindings.get(slot) {
            Some(new_key) if new_key == old_key => {}
            Some(_) => {
                rebound_instance_count += 1;
                recycled_node_ids.push(instance_node_id(&spec.list_node_id, *slot));
            }
            None => {
                retired_instance_count += 1;
                recycled_node_ids.push(instance_node_id(&spec.list_node_id, *slot));
            }
        }
    }
    let recycled = recycled_node_ids.iter().cloned().collect::<HashSet<_>>();
    let cleanup = interaction.reconcile_recycled_nodes(&recycled);

    let slot_by_key = new_bindings
        .iter()
        .map(|(slot, key)| (key.as_str(), *slot))
        .collect::<HashMap<_, _>>();
    let instances = visible_indices
        .iter()
        .filter_map(|index| {
            let item = &snapshot.items[*index];
            let slot = slot_by_key.get(item.item_key.as_str()).copied()?;
            Some(AuiVirtualListInstance {
                slot_index: slot,
                node_id: instance_node_id(&spec.list_node_id, slot),
                item_index: *index,
                item_key: item.item_key.clone(),
                top: prepared.tops[*index],
                extent: prepared.extents[*index],
            })
        })
        .collect::<Vec<_>>();

    next_state.extent_by_key = snapshot
        .items
        .iter()
        .zip(prepared.extents.iter().copied())
        .map(|(item, extent)| (item.item_key.clone(), extent))
        .collect();
    next_state.slot_bindings = new_bindings;
    let anchor = anchor_from_prepared(snapshot, &prepared, next_state.scroll_offset);
    let focused_instance_node_id = next_state.focused_key.as_ref().and_then(|focused| {
        next_state.slot_bindings.iter().find_map(|(slot, key)| {
            (key == focused).then(|| instance_node_id(&spec.list_node_id, *slot))
        })
    });
    let focused_key = next_state.focused_key.clone();
    let selected_key = next_state.selected_key.clone();
    let max_scroll_offset = (prepared.total_extent - spec.viewport_extent.max(0.0)).max(0.0);
    let reconciliation = AuiVirtualListReconciliation::from_cleanup(
        rebound_instance_count,
        retired_instance_count,
        recycled_node_ids,
        cleanup,
    );
    (
        AuiVirtualListWindow {
            schema_version: AUI_VIRTUAL_LIST_SCHEMA_VERSION.to_string(),
            list_node_id: spec.list_node_id.clone(),
            total_item_count: snapshot.items.len(),
            total_extent: prepared.total_extent,
            scroll_offset: next_state.scroll_offset,
            max_scroll_offset,
            first_materialized_index: visible_indices.first().copied(),
            last_materialized_index: visible_indices.last().copied(),
            instances,
            anchor,
            focused_key,
            focused_instance_node_id,
            selected_key,
            requested_key,
            capacity_limited,
            reconciliation,
        },
        next_state,
    )
}

fn anchor_for_offset(
    spec: &AuiVirtualListSpec,
    snapshot: &AuiVirtualListSnapshot,
    state: &AuiVirtualListState,
    offset: f32,
) -> Option<AuiVirtualListAnchor> {
    if snapshot.items.is_empty() {
        return None;
    }
    let mut cursor = 0.0_f32;
    for item in &snapshot.items {
        let extent = match spec.extent_mode {
            AuiVirtualListExtentMode::Fixed { item_extent } => item_extent,
            AuiVirtualListExtentMode::Variable {
                estimated_item_extent,
            } => state
                .extent_by_key
                .get(&item.item_key)
                .copied()
                .or(item.measured_extent)
                .unwrap_or(estimated_item_extent),
        };
        if offset < cursor + extent {
            return Some(AuiVirtualListAnchor {
                item_key: item.item_key.clone(),
                local_offset: (offset - cursor).max(0.0),
            });
        }
        cursor += extent;
    }
    snapshot.items.last().map(|item| AuiVirtualListAnchor {
        item_key: item.item_key.clone(),
        local_offset: 0.0,
    })
}

fn anchor_from_prepared(
    snapshot: &AuiVirtualListSnapshot,
    prepared: &PreparedList,
    offset: f32,
) -> Option<AuiVirtualListAnchor> {
    let index = prepared
        .tops
        .iter()
        .zip(&prepared.extents)
        .position(|(top, extent)| offset < *top + *extent)
        .or_else(|| snapshot.items.len().checked_sub(1))?;
    Some(AuiVirtualListAnchor {
        item_key: snapshot.items[index].item_key.clone(),
        local_offset: (offset - prepared.tops[index]).max(0.0),
    })
}

fn instance_node_id(list_node_id: &str, slot: usize) -> String {
    format!("{list_node_id}:virtual-item:{slot}")
}

fn valid_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn push_diagnostic(
    diagnostics: &mut Vec<AuiVirtualListDiagnostic>,
    code: impl Into<String>,
    item_key: Option<&str>,
    message: impl Into<String>,
) {
    diagnostics.push(AuiVirtualListDiagnostic {
        code: code.into(),
        item_key: item_key.map(str::to_string),
        message: message.into(),
    });
}

fn single_error(
    code: impl Into<String>,
    item_key: Option<&str>,
    message: impl Into<String>,
) -> AuiVirtualListError {
    let mut diagnostics = Vec::new();
    push_diagnostic(&mut diagnostics, code, item_key, message);
    AuiVirtualListError { diagnostics }
}
