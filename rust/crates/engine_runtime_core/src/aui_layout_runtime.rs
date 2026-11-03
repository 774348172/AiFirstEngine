//! Persistent C2 layout solver. Taffy is an algorithm detail and never crosses the AUI schema.
use crate::aui::{AuiComputedRect, AuiDocument, AuiNode, AuiValidationItem};
use crate::aui_layout_schema::{
    self, AlignContent, AlignItems, Dimension as AuiDimension, FlexDirection, FlexWrap,
    GridAutoFlow, GridTrack, JustifyContent, LayoutMode, LayoutVisibility, Overflow, Participation,
};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use taffy::geometry::Point;
use taffy::prelude::{
    auto, flex, length, line, max_content, min_content, minmax, percent, span, AvailableSpace,
    Display, GridPlacement, Layout, NodeId, Position, Rect, Size, Style,
};
use taffy::{
    compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_hidden_layout,
    compute_leaf_layout, compute_root_layout, Cache, CacheTree, LayoutFlexboxContainer,
    LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree, RunMode,
    TraversePartialTree,
};

pub const MAX_LAYOUT_MEASURE_CALLS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AuiAvailableSpace {
    Definite(f32),
    MinContent,
    MaxContent,
}

#[derive(Debug, Clone, Copy)]
pub struct AuiMeasureInput<'a> {
    pub node_id: &'a str,
    pub text: Option<&'a str>,
    pub known_width: Option<f32>,
    pub known_height: Option<f32>,
    pub available_width: AuiAvailableSpace,
    pub available_height: AuiAvailableSpace,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiMeasureOutput {
    pub width: f32,
    pub height: f32,
    pub first_baseline: Option<f32>,
}

pub trait AuiIntrinsicMeasure {
    fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput;

    fn layout_children(
        &mut self,
        _node_id: &str,
        _size: [f32; 2],
        _child_ids: &[String],
    ) -> Option<Vec<AuiCustomChildLayout>> {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuiCustomChildLayout {
    pub node_id: String,
    pub rect: AuiComputedRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiLayoutGeometry {
    pub node_id: String,
    pub rect: AuiComputedRect,
    pub visibility: LayoutVisibility,
    pub overflows_parent: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiLayoutSnapshot {
    pub revision: u64,
    pub canvas_id: String,
    pub nodes: Vec<AuiLayoutGeometry>,
    pub overflow_node_ids: Vec<String>,
}

impl AuiLayoutSnapshot {
    pub fn node(&self, node_id: &str) -> Option<&AuiLayoutGeometry> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuiLayoutStats {
    pub clean_snapshot_reused: bool,
    pub visited_nodes: usize,
    pub measure_calls: usize,
    pub cache_hits: usize,
    pub document_nodes_validated: usize,
    pub source_nodes_loaded: usize,
    pub property_nodes_updated: usize,
    pub geometry_nodes: usize,
    /// Snapshot geometry visited only to rebuild the retained overflow summary.
    #[serde(skip)]
    pub overflow_summary_nodes_scanned: usize,
    /// Overflow summary members whose state changed in this solve.
    #[serde(skip)]
    pub overflow_summary_entries_updated: usize,
}

struct RuntimeNode {
    source: Arc<AuiNode>,
    style: Style,
    children: Vec<usize>,
    parent: Option<usize>,
    cache: Cache,
    layout: Layout,
    manual_position: bool,
    reachable: bool,
    definite_size: DefiniteSize,
}

/// One detached immutable source/index shared by all mounted Canvas solvers. Keeping this
/// separate from the mutable prepared Document avoids creating an internal COW alias.
pub(crate) struct AuiLayoutSource {
    nodes: Vec<Arc<AuiNode>>,
    pub(crate) node_slots: Arc<HashMap<String, usize>>,
}

impl AuiLayoutSource {
    pub(crate) fn node(&self, slot: usize) -> Arc<AuiNode> {
        Arc::clone(&self.nodes[slot])
    }
    pub(crate) fn new(document: &AuiDocument) -> Self {
        Self {
            nodes: document.nodes.iter().cloned().map(Arc::new).collect(),
            node_slots: Arc::new(
                document
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(slot, node)| (node.node_id.clone(), slot))
                    .collect(),
            ),
        }
    }
}

struct RuntimeTree {
    nodes: Vec<RuntimeNode>,
    root: usize,
    geometry_dirty: HashSet<usize>,
    source_slots: HashMap<usize, usize>,
    node_slots: HashMap<String, usize>,
}

// SAFETY: This private tree owns its caches and shares immutable Send/Sync AuiNode sources.
// Its Taffy Styles are constructed
// exclusively below from numeric AUI dimensions; no calc pointer, borrowed data, callback or
// external Style enters this type. Taffy's CompactLength represents even plain numeric lengths
// as tagged *const () values, which prevents automatic Send. We never dereference those tags
// (resolve_calc_value also ignores its pointer). Moving the exclusively owned tree is safe;
// this does not permit simultaneous access or make a measurer Send/Sync.
unsafe impl Send for RuntimeTree {}

pub struct AuiLayoutSolver {
    tree: Option<RuntimeTree>,
    source: Option<AuiDocument>,
    canvas_id: Option<String>,
    revision: Option<u64>,
    snapshot: Option<AuiLayoutSnapshot>,
    snapshot_slots: Option<SnapshotSlots>,
    pending_dirty: HashSet<String>,
    last_stats: AuiLayoutStats,
    measure_budget: usize,
    root_rect: Option<AuiComputedRect>,
    instance_updates: HashSet<usize>,
    instance_layout_dirty: HashSet<usize>,
    instance_configure_dirty: HashSet<usize>,
    last_geometry_slots: Vec<usize>,
}

impl Default for AuiLayoutSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl AuiLayoutSolver {
    pub fn new() -> Self {
        Self {
            tree: None,
            source: None,
            canvas_id: None,
            revision: None,
            snapshot: None,
            snapshot_slots: None,
            pending_dirty: HashSet::new(),
            last_stats: AuiLayoutStats::default(),
            measure_budget: MAX_LAYOUT_MEASURE_CALLS,
            root_rect: None,
            instance_updates: HashSet::new(),
            instance_layout_dirty: HashSet::new(),
            instance_configure_dirty: HashSet::new(),
            last_geometry_slots: Vec::new(),
        }
    }

    /// Lowers the per-solve budget for constrained hosts and deterministic tests.
    pub fn set_measure_budget(&mut self, budget: usize) {
        self.measure_budget = budget.min(MAX_LAYOUT_MEASURE_CALLS);
    }

    pub fn last_stats(&self) -> AuiLayoutStats {
        self.last_stats
    }

    /// Instance mounting already validated the immutable source, including legacy structure.
    pub(crate) fn for_shared_instance(
        document: &AuiDocument,
        canvas_id: &str,
        source: &AuiLayoutSource,
    ) -> Result<Self, Vec<AuiValidationItem>> {
        let canvas = document
            .canvases
            .iter()
            .find(|c| c.canvas_id == canvas_id)
            .ok_or_else(|| {
                vec![diagnostic(
                    "aui_layout_canvas_missing",
                    None,
                    "The requested canvas does not exist in the AUI document.",
                )]
            })?;
        let root_rect = canvas.resolved_layout_root_rect();
        Ok(Self {
            tree: Some(build_shared_tree(document, canvas_id, root_rect, source)?),
            canvas_id: Some(canvas_id.into()),
            root_rect: Some(root_rect),
            ..Self::new()
        })
    }

    /// Slot identities belong to the enclosing instance; no string search or source diff is needed.
    pub(crate) fn update_instance_node(
        &mut self,
        slot: usize,
        text: Option<&str>,
        visibility: LayoutVisibility,
    ) {
        let tree = self.tree.as_mut().expect("mounted instance solver");
        let Some(&slot) = tree.source_slots.get(&slot) else {
            return;
        };
        let node = &mut tree.nodes[slot];
        if !node.reachable {
            return;
        }
        if let Some(text) = text {
            if node.source.text.as_deref() != Some(text) {
                Arc::make_mut(&mut node.source).text = Some(text.into());
                self.instance_updates.insert(slot);
                self.instance_layout_dirty.insert(slot);
            }
        }
        let old = node.source.layout_visibility.unwrap_or_default();
        if old != visibility {
            Arc::make_mut(&mut node.source).layout_visibility = Some(visibility);
            self.instance_updates.insert(slot);
            if old == LayoutVisibility::Collapsed || visibility == LayoutVisibility::Collapsed {
                node.style.display = if visibility == LayoutVisibility::Collapsed {
                    Display::None
                } else if matches!(
                    node.source
                        .layout
                        .as_ref()
                        .map_or(LayoutMode::Anchor, |l| l.mode),
                    LayoutMode::Grid | LayoutMode::Overlay
                ) {
                    Display::Grid
                } else {
                    Display::Flex
                };
                self.instance_layout_dirty.insert(slot);
            }
        }
    }

    pub(crate) fn resident_node_count(&self) -> usize {
        self.tree.as_ref().map_or(0, |tree| tree.nodes.len())
    }

    pub(crate) fn rebase_source(&mut self, source: &AuiLayoutSource) {
        if let Some(tree) = &mut self.tree {
            for (&global, &local) in &tree.source_slots {
                tree.nodes[local].source = source.node(global);
            }
        }
    }

    /// Reconcile structure by stable node ID. A changed root's topology returns false; an
    /// independent root can remap global source slots without discarding its Taffy caches.
    pub(crate) fn reconcile_source_slots(
        &mut self,
        document: &AuiDocument,
        slots: &HashMap<String, usize>,
    ) -> bool {
        let Some(tree) = &mut self.tree else {
            return false;
        };
        let Some(canvas) = document
            .canvases
            .iter()
            .find(|canvas| Some(canvas.canvas_id.as_str()) == self.canvas_id.as_deref())
        else {
            return false;
        };
        if tree.nodes[tree.root].source.node_id != canvas.root_node {
            return false;
        }
        let mut mapped = HashMap::with_capacity(tree.nodes.len());
        for (local, node) in tree.nodes.iter().enumerate() {
            let Some(&global) = slots.get(&node.source.node_id) else {
                return false;
            };
            let new = &document.nodes[global];
            if node.source.children != new.children
                || (local != tree.root && node.source.parent != new.parent)
            {
                return false;
            }
            mapped.insert(global, local);
        }
        tree.source_slots = mapped;
        true
    }

    pub(crate) fn update_instance_source_node(&mut self, global: usize, source: Arc<AuiNode>) {
        let tree = self.tree.as_mut().expect("mounted solver");
        let Some(&slot) = tree.source_slots.get(&global) else {
            return;
        };
        let old = &tree.nodes[slot].source;
        let configure = old.rect != source.rect
            || old.layout != source.layout
            || old.slot != source.slot
            || old.overflow != source.overflow
            || (old.layout_visibility == Some(LayoutVisibility::Collapsed))
                != (source.layout_visibility == Some(LayoutVisibility::Collapsed));
        let measure_changed = measurement_inputs_changed(old, &source);
        let visibility_changed = old.layout_visibility != source.layout_visibility;
        if !configure && !measure_changed && !visibility_changed {
            tree.nodes[slot].source = source;
            return;
        }
        tree.nodes[slot].source = source;
        self.instance_updates.insert(slot);
        if configure {
            self.instance_configure_dirty.insert(slot);
            tree.geometry_dirty.insert(slot);
        }
        if configure || measure_changed {
            self.instance_layout_dirty.insert(slot);
        }
    }

    /// Install all source writes before deriving descendant constraints. A valid multi-node
    /// edit must not be checked against another node's values from the previous frame.
    pub(crate) fn configure_instance_inputs(&mut self) -> Result<(), Vec<AuiValidationItem>> {
        let tree = self.tree.as_mut().expect("mounted solver");
        let mut roots: Vec<_> = self
            .instance_configure_dirty
            .iter()
            .copied()
            .filter(|&slot| {
                let mut parent = tree.nodes[slot].parent;
                while let Some(ancestor) = parent {
                    if self.instance_configure_dirty.contains(&ancestor) {
                        return false;
                    }
                    parent = tree.nodes[ancestor].parent;
                }
                true
            })
            .collect();
        roots.sort_unstable();
        for slot in roots {
            let parent = tree.nodes[slot].parent;
            let mode = parent.map(|parent| {
                tree.nodes[parent]
                    .source
                    .layout
                    .as_ref()
                    .map_or(LayoutMode::Anchor, |layout| layout.mode)
            });
            let direction = parent.map(|parent| layout_direction(&tree.nodes[parent].source));
            let size = parent
                .map(|parent| tree.nodes[parent].definite_size)
                .unwrap_or(DefiniteSize {
                    width: Some(self.root_rect.unwrap().width),
                    height: Some(self.root_rect.unwrap().height),
                });
            configure_subtree(tree, slot, mode, direction, size, slot == tree.root)?;
        }
        self.instance_configure_dirty.clear();
        Ok(())
    }

    pub(crate) fn update_instance_root_rect(&mut self, rect: AuiComputedRect) {
        if self.root_rect == Some(rect) {
            return;
        }
        let tree = self.tree.as_mut().expect("mounted solver");
        self.root_rect = Some(rect);
        self.instance_updates.insert(tree.root);
        self.instance_layout_dirty.insert(tree.root);
        self.instance_configure_dirty.insert(tree.root);
        tree.geometry_dirty.insert(tree.root);
    }

    pub(crate) fn invalidate_instance_measurements(&mut self) {
        if let Some(tree) = &mut self.tree {
            for (slot, node) in tree.nodes.iter_mut().enumerate() {
                node.cache.clear();
                self.instance_updates.insert(slot);
                self.instance_layout_dirty.insert(slot);
            }
        }
    }

    pub(crate) fn instance_snapshot(&self) -> Option<&AuiLayoutSnapshot> {
        self.snapshot.as_ref()
    }

    pub(crate) fn last_geometry_node_ids(&self) -> impl Iterator<Item = &str> {
        self.last_geometry_slots.iter().filter_map(|&slot| {
            self.snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.nodes.get(slot))
                .map(|node| node.node_id.as_str())
        })
    }

    pub(crate) fn solve_instance(
        &mut self,
        measure: &mut dyn AuiIntrinsicMeasure,
    ) -> Result<&AuiLayoutSnapshot, Vec<AuiValidationItem>> {
        self.configure_instance_inputs()?;
        self.last_stats = AuiLayoutStats::default();
        self.last_geometry_slots.clear();
        // As in solve(), decide reuse before borrowing the snapshot for the returned lifetime.
        let clean = self.snapshot.is_some() && self.instance_updates.is_empty();
        if clean {
            self.last_stats.clean_snapshot_reused = true;
            return Ok(self.snapshot.as_ref().expect("checked snapshot"));
        }
        let tree = self.tree.as_mut().expect("mounted instance solver");
        let root_rect = self.root_rect.expect("mounted root");
        if self.snapshot.is_none() || !self.instance_layout_dirty.is_empty() {
            self.last_stats = if self.snapshot.is_none() {
                run_solver(tree, measure, self.measure_budget, root_rect)?
            } else {
                run_dirty_solver(
                    tree,
                    &self.instance_layout_dirty,
                    measure,
                    self.measure_budget,
                    root_rect,
                )?
            };
        }
        let revision = self.revision.unwrap_or_default().saturating_add(1);
        self.last_stats.source_nodes_loaded = if self.snapshot.is_none() {
            tree.nodes.len()
        } else {
            0
        };
        self.last_stats.property_nodes_updated = self.instance_updates.len();
        if let (Some(snapshot), Some(slots)) = (&mut self.snapshot, &mut self.snapshot_slots) {
            let (updated, overflow_summary_nodes_scanned, overflow_summary_entries_updated) =
                slots.update(tree, snapshot, &self.instance_updates, root_rect);
            self.last_geometry_slots = updated;
            self.last_stats.geometry_nodes = self.last_geometry_slots.len();
            self.last_stats.overflow_summary_nodes_scanned = overflow_summary_nodes_scanned;
            self.last_stats.overflow_summary_entries_updated = overflow_summary_entries_updated;
            snapshot.revision = revision;
        } else {
            let snapshot = build_snapshot(
                tree,
                self.canvas_id.as_deref().expect("mounted canvas"),
                revision,
                root_rect,
            );
            self.last_stats.geometry_nodes = snapshot.nodes.len();
            self.last_geometry_slots = (0..snapshot.nodes.len()).collect();
            self.snapshot_slots = Some(SnapshotSlots::new(tree, &snapshot));
            self.snapshot = Some(snapshot);
        }
        tree.geometry_dirty.clear();
        self.revision = Some(revision);
        self.instance_updates.clear();
        self.instance_layout_dirty.clear();
        Ok(self.snapshot.as_ref().expect("stored snapshot"))
    }

    pub fn mark_dirty(&mut self, node_id: &str) -> Result<(), String> {
        if let Some(tree) = &mut self.tree {
            let Some(&index) = tree.node_slots.get(node_id) else {
                return Err(format!("Unknown AUI layout node '{node_id}'"));
            };
            invalidate_to_root(tree, index);
        }
        self.pending_dirty.insert(node_id.to_owned());
        Ok(())
    }

    pub fn solve<'a>(
        &'a mut self,
        document: &AuiDocument,
        canvas_id: &str,
        revision: u64,
        measure: &mut dyn AuiIntrinsicMeasure,
    ) -> Result<&'a AuiLayoutSnapshot, Vec<AuiValidationItem>> {
        let validation = aui_layout_schema::validate(document);
        if !validation.is_empty() {
            return Err(validation);
        }
        let Some(canvas) = document
            .canvases
            .iter()
            .find(|canvas| canvas.canvas_id == canvas_id)
        else {
            return Err(vec![diagnostic(
                "aui_layout_canvas_missing",
                None,
                "The requested canvas does not exist in the AUI document.",
            )]);
        };

        let clean = self.revision == Some(revision)
            && self.canvas_id.as_deref() == Some(canvas_id)
            && self.root_rect == Some(canvas.resolved_layout_root_rect())
            && self.pending_dirty.is_empty()
            && self.snapshot.is_some();
        if clean {
            self.last_stats = AuiLayoutStats {
                clean_snapshot_reused: true,
                document_nodes_validated: document.nodes.len(),
                ..Default::default()
            };
            return Ok(self.snapshot.as_ref().expect("checked snapshot"));
        }

        let root_rect = canvas.resolved_layout_root_rect();
        let root_constraint_changed = self.root_rect != Some(root_rect);
        let can_update_measure_only = !root_constraint_changed
            && self.canvas_id.as_deref() == Some(canvas_id)
            && self
                .source
                .as_ref()
                .is_some_and(|old| only_measure_inputs_changed(old, document));
        let rebuild = self.tree.is_none()
            || self.canvas_id.as_deref() != Some(canvas_id)
            || root_constraint_changed
            || (!can_update_measure_only && self.source.as_ref() != Some(document));
        if rebuild {
            self.tree = Some(build_tree(document, canvas_id, root_rect)?);
        } else if can_update_measure_only {
            if let Some(tree) = self.tree.as_mut() {
                for (index, source) in document.nodes.iter().enumerate() {
                    let Some(&local) = tree.source_slots.get(&index) else {
                        continue;
                    };
                    if measurement_inputs_changed(&tree.nodes[local].source, source) {
                        tree.nodes[local].source = Arc::new(source.clone());
                        invalidate_to_root(tree, local);
                    }
                }
            }
        }
        if let Some(tree) = &mut self.tree {
            let dirty = self.pending_dirty.drain().collect::<Vec<_>>();
            for node_id in dirty {
                if let Some(&index) = tree.node_slots.get(&node_id) {
                    invalidate_to_root(tree, index);
                }
            }
        }

        let tree = self.tree.as_mut().expect("tree was built");
        let mut stats = run_solver(tree, measure, self.measure_budget, root_rect)?;

        let snapshot = build_snapshot(tree, canvas_id, revision, root_rect);
        stats.document_nodes_validated = document.nodes.len();
        stats.source_nodes_loaded = if rebuild { tree.nodes.len() } else { 0 };
        stats.geometry_nodes = snapshot.nodes.len();
        tree.geometry_dirty.clear();
        self.snapshot_slots = None;
        self.source = Some(document.clone());
        self.canvas_id = Some(canvas_id.to_owned());
        self.revision = Some(revision);
        self.root_rect = Some(root_rect);
        self.snapshot = Some(snapshot);
        self.last_stats = stats;
        Ok(self.snapshot.as_ref().expect("snapshot was stored"))
    }
}

fn run_solver(
    tree: &mut RuntimeTree,
    measure: &mut dyn AuiIntrinsicMeasure,
    budget: usize,
    root_rect: AuiComputedRect,
) -> Result<AuiLayoutStats, Vec<AuiValidationItem>> {
    run_solver_at(tree, tree.root, measure, budget, root_rect)
}

fn run_solver_at(
    tree: &mut RuntimeTree,
    root: usize,
    measure: &mut dyn AuiIntrinsicMeasure,
    budget: usize,
    root_rect: AuiComputedRect,
) -> Result<AuiLayoutStats, Vec<AuiValidationItem>> {
    let (stats, budget_exceeded, invalid_measure) = {
        let mut adapter = SolverAdapter::new(tree, measure, budget);
        compute_root_layout(
            &mut adapter,
            NodeId::from(root),
            Size {
                width: AvailableSpace::Definite(root_rect.width),
                height: AvailableSpace::Definite(root_rect.height),
            },
        );
        (
            adapter.stats(),
            adapter.budget_exceeded.get(),
            adapter.invalid_measure.get(),
        )
    };
    if budget_exceeded || invalid_measure {
        // Failed measurements may have populated descendant caches. Never reuse these on retry.
        for node in &mut tree.nodes {
            node.cache.clear();
        }
        return Err(vec![if budget_exceeded {
            diagnostic(
                "aui_layout_measure_budget",
                None,
                "Intrinsic measurement exceeded the per-solve budget.",
            )
        } else {
            diagnostic(
                "aui_layout_measure_invalid",
                None,
                "Intrinsic measurement returned non-finite or negative metrics.",
            )
        }]);
    }
    Ok(stats)
}

fn run_dirty_solver(
    tree: &mut RuntimeTree,
    dirty: &HashSet<usize>,
    measure: &mut dyn AuiIntrinsicMeasure,
    budget: usize,
    root_rect: AuiComputedRect,
) -> Result<AuiLayoutStats, Vec<AuiValidationItem>> {
    let mut roots = HashSet::new();
    for &slot in dirty {
        // Definite manual/out-of-flow regions have placement and extent independent of
        // descendant measurements. Flow/content/baseline and custom dependencies propagate.
        let mut ancestor = Some(slot);
        let mut boundary = None;
        let mut custom_or_hidden = false;
        while let Some(parent) = ancestor {
            let node = &tree.nodes[parent];
            custom_or_hidden |= node.source.kind == crate::aui::AuiNodeKind::Custom
                || node.style.display == Display::None;
            if boundary.is_none()
                && node.manual_position
                && node.definite_size.width.is_some()
                && node.definite_size.height.is_some()
            {
                boundary = Some(parent);
            }
            ancestor = node.parent;
        }
        let root = if custom_or_hidden {
            tree.root
        } else {
            boundary.unwrap_or(tree.root)
        };
        let mut index = slot;
        loop {
            tree.nodes[index].cache.clear();
            if index == root {
                break;
            }
            index = tree.nodes[index].parent.expect("boundary is an ancestor");
        }
        roots.insert(root);
    }
    let mut roots: Vec<_> = roots
        .iter()
        .copied()
        .filter(|&root| {
            let mut parent = tree.nodes[root].parent;
            while let Some(ancestor) = parent {
                if roots.contains(&ancestor) {
                    return false;
                }
                parent = tree.nodes[ancestor].parent;
            }
            true
        })
        .collect();
    roots.sort_unstable();
    let mut total = AuiLayoutStats::default();
    for root in roots {
        let old_location = tree.nodes[root].layout.location;
        let rect = if root == tree.root {
            root_rect
        } else {
            AuiComputedRect {
                x: 0.,
                y: 0.,
                width: tree.nodes[root]
                    .definite_size
                    .width
                    .expect("definite boundary"),
                height: tree.nodes[root]
                    .definite_size
                    .height
                    .expect("definite boundary"),
            }
        };
        let result = run_solver_at(
            tree,
            root,
            measure,
            budget.saturating_sub(total.measure_calls),
            rect,
        );
        // compute_root_layout sets the local origin; keep placement owned by the parent.
        tree.nodes[root].layout.location = old_location;
        let stats = result?;
        total.visited_nodes += stats.visited_nodes;
        total.measure_calls += stats.measure_calls;
        total.cache_hits += stats.cache_hits;
    }
    Ok(total)
}

fn only_measure_inputs_changed(old: &AuiDocument, new: &AuiDocument) -> bool {
    if old.canvases != new.canvases || old.nodes.len() != new.nodes.len() {
        return false;
    }
    old.nodes.iter().zip(&new.nodes).all(|(a, b)| {
        a.node_id == b.node_id
            && a.parent == b.parent
            && a.children == b.children
            && a.layout == b.layout
            && a.slot == b.slot
            && a.rect == b.rect
            && a.layout_visibility == b.layout_visibility
            && a.overflow == b.overflow
    })
}

fn measure_inputs(
    node: &AuiNode,
) -> (
    &crate::aui::AuiNodeKind,
    &Option<String>,
    Option<f32>,
    Option<&crate::aui::AuiFontStyle>,
    &Option<aui_layout_schema::TextLayoutSpec>,
) {
    (
        &node.kind,
        &node.text,
        node.style.as_ref().and_then(|style| style.font_size),
        node.style.as_ref().and_then(|style| style.font.as_ref()),
        &node.text_layout,
    )
}

fn measurement_inputs_changed(old: &AuiNode, new: &AuiNode) -> bool {
    measure_inputs(old) != measure_inputs(new)
        || old.text_spans != new.text_spans
        || old.placeholder != new.placeholder
        // Accepted measure/layout products are layout inputs. Paint, callback identity and
        // property revisions cannot change constraints until those products actually change.
        || old.custom_runtime.as_ref().and_then(|output| output.measure.as_ref())
            != new.custom_runtime.as_ref().and_then(|output| output.measure.as_ref())
        || old.custom_runtime.as_ref().map(|output| output.child_rects.as_slice()).unwrap_or_default()
            != new.custom_runtime.as_ref().map(|output| output.child_rects.as_slice()).unwrap_or_default()
}

fn diagnostic(code: &str, node_id: Option<String>, message: &str) -> AuiValidationItem {
    AuiValidationItem::error(
        code,
        node_id,
        None,
        message,
        "Fix the C2 layout input before retrying the solver.",
    )
}

fn build_tree(
    document: &AuiDocument,
    canvas_id: &str,
    root_rect: AuiComputedRect,
) -> Result<RuntimeTree, Vec<AuiValidationItem>> {
    build_shared_tree(
        document,
        canvas_id,
        root_rect,
        &AuiLayoutSource::new(document),
    )
}

fn build_shared_tree(
    document: &AuiDocument,
    canvas_id: &str,
    root_rect: AuiComputedRect,
    source: &AuiLayoutSource,
) -> Result<RuntimeTree, Vec<AuiValidationItem>> {
    let canvas = document
        .canvases
        .iter()
        .find(|canvas| canvas.canvas_id == canvas_id)
        .expect("canvas checked by caller");
    let ids = &source.node_slots;
    let mut globals = Vec::new();
    let mut pending = vec![ids[&canvas.root_node]];
    while let Some(global) = pending.pop() {
        globals.push(global);
        pending.extend(
            document.nodes[global]
                .children
                .iter()
                .rev()
                .map(|id| ids[id]),
        );
    }
    let source_slots: HashMap<_, _> = globals
        .iter()
        .enumerate()
        .map(|(local, &global)| (global, local))
        .collect();
    let node_slots: HashMap<_, _> = globals
        .iter()
        .enumerate()
        .map(|(local, &global)| (document.nodes[global].node_id.clone(), local))
        .collect();
    let root = 0;
    let mut nodes = globals
        .iter()
        .map(|&global| RuntimeNode {
            source: Arc::clone(&source.nodes[global]),
            style: Style::default(),
            children: Vec::new(),
            parent: None,
            cache: Cache::new(),
            layout: Layout::with_order(0),
            manual_position: false,
            reachable: false,
            definite_size: DefiniteSize::default(),
        })
        .collect::<Vec<_>>();
    for (index, &global) in globals.iter().enumerate() {
        let node = &document.nodes[global];
        nodes[index].children = node.children.iter().map(|id| node_slots[id]).collect();
        nodes[index].parent = if index == root {
            None
        } else {
            node.parent.as_ref().map(|id| node_slots[id])
        };
    }
    let mut tree = RuntimeTree {
        nodes,
        root,
        geometry_dirty: HashSet::new(),
        source_slots,
        node_slots,
    };
    configure_subtree(
        &mut tree,
        root,
        None,
        None,
        DefiniteSize {
            width: Some(root_rect.width),
            height: Some(root_rect.height),
        },
        true,
    )?;
    Ok(tree)
}

fn configure_subtree(
    tree: &mut RuntimeTree,
    index: usize,
    parent_mode: Option<LayoutMode>,
    parent_direction: Option<FlexDirection>,
    parent_size: DefiniteSize,
    is_root: bool,
) -> Result<(), Vec<AuiValidationItem>> {
    let source = tree.nodes[index].source.clone();
    let participation = source
        .slot
        .as_ref()
        .map_or(Participation::Flow, |slot| slot.participation);
    let manual = !is_root
        && (parent_mode == Some(LayoutMode::Anchor) || participation == Participation::OutOfFlow);
    let mut local_legacy = None;
    if manual {
        local_legacy = Some(resolve_legacy_rect(&source, parent_size)?);
    }
    reject_indefinite_percent(&source, parent_mode, parent_direction, parent_size)?;
    let definite_size = if is_root {
        parent_size
    } else if let Some(rect) = local_legacy {
        DefiniteSize {
            width: Some(rect.width),
            height: Some(rect.height),
        }
    } else {
        source
            .slot
            .as_ref()
            .map_or(DefiniteSize::default(), |slot| DefiniteSize {
                width: resolve_definite(slot.width, parent_size.width),
                height: resolve_definite(slot.height, parent_size.height),
            })
    };
    let root_size = is_root.then(|| {
        (
            parent_size.width.expect("canvas width is definite"),
            parent_size.height.expect("canvas height is definite"),
        )
    });
    let style = style_for(
        &source,
        parent_mode,
        parent_direction,
        parent_size,
        root_size,
        local_legacy,
    );
    if tree.nodes[index].style != style
        || tree.nodes[index].manual_position != manual
        || tree.nodes[index].definite_size != definite_size
    {
        tree.nodes[index].cache.clear();
        tree.geometry_dirty.insert(index);
    }
    tree.nodes[index].style = style;
    tree.nodes[index].definite_size = definite_size;
    tree.nodes[index].manual_position = manual;
    tree.nodes[index].reachable = true;
    let mode = source
        .layout
        .as_ref()
        .map_or(LayoutMode::Anchor, |layout| layout.mode);
    let children = tree.nodes[index].children.clone();
    for child in children {
        configure_subtree(
            tree,
            child,
            Some(mode),
            Some(layout_direction(&source)),
            definite_size,
            false,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct DefiniteSize {
    width: Option<f32>,
    height: Option<f32>,
}

fn resolve_legacy_rect(
    node: &AuiNode,
    parent_size: DefiniteSize,
) -> Result<AuiComputedRect, Vec<AuiValidationItem>> {
    let needs_width = node.rect.anchor_min.x != 0.0 || node.rect.anchor_max.x != 0.0;
    let needs_height = node.rect.anchor_min.y != 0.0 || node.rect.anchor_max.y != 0.0;
    if (needs_width && parent_size.width.is_none())
        || (needs_height && parent_size.height.is_none())
    {
        return Err(vec![diagnostic(
            "aui_layout_indefinite_anchor_basis",
            Some(node.node_id.clone()),
            "Anchor or out-of-flow placement requires a definite parent size on every referenced axis.",
        )]);
    }
    Ok(node.rect.resolve(AuiComputedRect {
        x: 0.0,
        y: 0.0,
        width: parent_size.width.unwrap_or(0.0),
        height: parent_size.height.unwrap_or(0.0),
    }))
}

fn reject_indefinite_percent(
    node: &AuiNode,
    parent_mode: Option<LayoutMode>,
    parent_direction: Option<FlexDirection>,
    parent_size: DefiniteSize,
) -> Result<(), Vec<AuiValidationItem>> {
    let Some(slot) = &node.slot else {
        return Ok(());
    };
    let main_basis = match parent_mode {
        Some(LayoutMode::Column) => parent_size.height,
        Some(LayoutMode::Row) => parent_size.width,
        Some(LayoutMode::Flex) => match parent_direction.unwrap_or_default() {
            FlexDirection::Column | FlexDirection::ColumnReverse => parent_size.height,
            FlexDirection::Row | FlexDirection::RowReverse => parent_size.width,
        },
        _ => Some(0.0),
    };
    let invalid = (matches!(slot.width, AuiDimension::Percent { .. })
        && parent_size.width.is_none())
        || (matches!(slot.height, AuiDimension::Percent { .. }) && parent_size.height.is_none())
        || (matches!(slot.flex_basis, AuiDimension::Percent { .. }) && main_basis.is_none());
    if invalid {
        return Err(vec![diagnostic(
            "aui_layout_indefinite_percent_basis",
            Some(node.node_id.clone()),
            "Percentage sizing requires a definite parent content-box basis.",
        )]);
    }
    Ok(())
}

fn resolve_definite(value: AuiDimension, parent: Option<f32>) -> Option<f32> {
    match value {
        AuiDimension::Fixed { value } => Some(value),
        AuiDimension::Fill => parent,
        AuiDimension::Percent { value } => parent.map(|parent| parent * value),
        AuiDimension::Content => None,
    }
}

fn style_for(
    node: &AuiNode,
    parent_mode: Option<LayoutMode>,
    parent_direction: Option<FlexDirection>,
    parent_size: DefiniteSize,
    root_size: Option<(f32, f32)>,
    legacy: Option<AuiComputedRect>,
) -> Style {
    let layout = node.layout.clone().unwrap_or_default();
    let slot = node.slot.clone().unwrap_or_default();
    let mut style = Style {
        display: if node.layout_visibility == Some(LayoutVisibility::Collapsed) {
            Display::None
        } else if matches!(layout.mode, LayoutMode::Grid | LayoutMode::Overlay) {
            Display::Grid
        } else {
            Display::Flex
        },
        size: Size {
            width: dimension(slot.width, parent_size.width),
            height: dimension(slot.height, parent_size.height),
        },
        min_size: Size {
            width: length(slot.min_width),
            height: length(slot.min_height),
        },
        max_size: Size {
            width: slot.max_width.map_or_else(auto, length),
            height: slot.max_height.map_or_else(auto, length),
        },
        margin: Rect {
            left: length(slot.margin.left),
            right: length(slot.margin.right),
            top: length(slot.margin.top),
            bottom: length(slot.margin.bottom),
        },
        padding: Rect {
            left: length(layout.padding.left),
            right: length(layout.padding.right),
            top: length(layout.padding.top),
            bottom: length(layout.padding.bottom),
        },
        gap: Size {
            width: length(layout.gap.column),
            height: length(layout.gap.row),
        },
        flex_direction: map_direction(match layout.mode {
            LayoutMode::Row => FlexDirection::Row,
            LayoutMode::Column => FlexDirection::Column,
            _ => layout.direction,
        }),
        flex_wrap: map_wrap(layout.wrap),
        flex_basis: dimension(
            slot.flex_basis,
            match parent_mode {
                Some(LayoutMode::Column) => parent_size.height,
                Some(LayoutMode::Flex)
                    if matches!(
                        parent_direction.unwrap_or_default(),
                        FlexDirection::Column | FlexDirection::ColumnReverse
                    ) =>
                {
                    parent_size.height
                }
                _ => parent_size.width,
            },
        ),
        flex_grow: slot.flex_grow,
        flex_shrink: slot.flex_shrink,
        justify_content: Some(map_justify(layout.justify_content)),
        align_items: Some(map_align_items(layout.align_items)),
        align_self: slot.align_self.map(map_align_self),
        align_content: Some(map_align_content(layout.align_content)),
        grid_template_columns: layout.grid_columns.iter().map(track).collect(),
        grid_template_rows: layout.grid_rows.iter().map(track).collect(),
        grid_auto_flow: match layout.grid_auto_flow {
            GridAutoFlow::Row => taffy::style::GridAutoFlow::Row,
            GridAutoFlow::Column => taffy::style::GridAutoFlow::Column,
        },
        grid_column: placement(slot.grid_column),
        grid_row: placement(slot.grid_row),
        overflow: Point {
            x: map_overflow(node.overflow.unwrap_or_default()),
            y: map_overflow(node.overflow.unwrap_or_default()),
        },
        ..Default::default()
    };
    if parent_mode == Some(LayoutMode::Overlay) && slot.participation == Participation::Flow {
        style.grid_column = line(1);
        style.grid_row = line(1);
    }
    if let Some((width, height)) = root_size {
        style.size = Size {
            width: length(width),
            height: length(height),
        };
        style.flex_shrink = 0.0;
    }
    if let Some(rect) = legacy {
        style.position = Position::Absolute;
        style.inset.left = length(rect.x);
        style.inset.top = length(rect.y);
        style.size = Size {
            width: length(rect.width),
            height: length(rect.height),
        };
        style.flex_shrink = 0.0;
    }
    style
}

fn layout_direction(node: &AuiNode) -> FlexDirection {
    let layout = node.layout.as_ref();
    match layout.map_or(LayoutMode::Anchor, |layout| layout.mode) {
        LayoutMode::Row => FlexDirection::Row,
        LayoutMode::Column => FlexDirection::Column,
        _ => layout.map_or_else(FlexDirection::default, |layout| layout.direction),
    }
}

fn dimension(value: AuiDimension, basis: Option<f32>) -> taffy::style::Dimension {
    match value {
        AuiDimension::Content => auto(),
        AuiDimension::Fixed { value } => length(value),
        AuiDimension::Fill if basis.is_some() => percent(1.0),
        AuiDimension::Fill => auto(),
        AuiDimension::Percent { value } => percent(value),
    }
}

fn map_direction(value: FlexDirection) -> taffy::style::FlexDirection {
    match value {
        FlexDirection::Row => taffy::style::FlexDirection::Row,
        FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
        FlexDirection::Column => taffy::style::FlexDirection::Column,
        FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    }
}

fn map_wrap(value: FlexWrap) -> taffy::style::FlexWrap {
    match value {
        FlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
    }
}

fn map_justify(value: JustifyContent) -> taffy::style::JustifyContent {
    use taffy::style::JustifyContent as T;
    match value {
        JustifyContent::Start => T::START,
        JustifyContent::End => T::END,
        JustifyContent::Center => T::CENTER,
        JustifyContent::SpaceBetween => T::SPACE_BETWEEN,
        JustifyContent::SpaceAround => T::SPACE_AROUND,
        JustifyContent::SpaceEvenly => T::SPACE_EVENLY,
    }
}

fn map_align_items(value: AlignItems) -> taffy::style::AlignItems {
    use taffy::style::AlignItems as T;
    match value {
        AlignItems::Stretch => T::STRETCH,
        AlignItems::Start => T::START,
        AlignItems::End => T::END,
        AlignItems::Center => T::CENTER,
        AlignItems::Baseline => T::BASELINE,
    }
}

fn map_align_self(value: AlignItems) -> taffy::style::AlignSelf {
    map_align_items(value)
}

fn map_align_content(value: AlignContent) -> taffy::style::AlignContent {
    use taffy::style::AlignContent as T;
    match value {
        AlignContent::Stretch => T::STRETCH,
        AlignContent::Start => T::START,
        AlignContent::End => T::END,
        AlignContent::Center => T::CENTER,
        AlignContent::SpaceBetween => T::SPACE_BETWEEN,
        AlignContent::SpaceAround => T::SPACE_AROUND,
        AlignContent::SpaceEvenly => T::SPACE_EVENLY,
    }
}

fn map_overflow(value: Overflow) -> taffy::style::Overflow {
    match value {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Clip => taffy::style::Overflow::Clip,
        Overflow::Scroll => taffy::style::Overflow::Scroll,
    }
}

fn track(value: &GridTrack) -> taffy::style::GridTemplateComponent<String> {
    match value {
        GridTrack::Content | GridTrack::MaxContent => max_content(),
        GridTrack::MinContent => min_content(),
        GridTrack::Fixed { value } => length(*value),
        GridTrack::Fr { value } => flex(*value),
        GridTrack::Minmax { min, max } => minmax(track_min(min), track_max(max)),
    }
}

fn track_min(value: &GridTrack) -> taffy::style::MinTrackSizingFunction {
    match value {
        GridTrack::Fixed { value } => length(*value),
        GridTrack::MinContent => min_content(),
        GridTrack::Content | GridTrack::MaxContent => max_content(),
        GridTrack::Fr { .. } | GridTrack::Minmax { .. } => min_content(),
    }
}

fn track_max(value: &GridTrack) -> taffy::style::MaxTrackSizingFunction {
    match value {
        GridTrack::Fixed { value } => length(*value),
        GridTrack::MinContent => min_content(),
        GridTrack::Content | GridTrack::MaxContent => max_content(),
        GridTrack::Fr { value } => taffy::style_helpers::fr(*value),
        GridTrack::Minmax { .. } => max_content(),
    }
}

fn placement(
    value: aui_layout_schema::GridPlacement,
) -> taffy::geometry::Line<GridPlacement<String>> {
    taffy::geometry::Line {
        start: value
            .start
            .map_or(GridPlacement::Auto, |start| line(start as i16)),
        end: if value.span > 1 {
            span(value.span)
        } else {
            GridPlacement::Auto
        },
    }
}

struct SolverAdapter<'a> {
    tree: &'a mut RuntimeTree,
    measure: &'a mut dyn AuiIntrinsicMeasure,
    measure_calls: Cell<usize>,
    visited_nodes: Cell<usize>,
    cache_hits: Cell<usize>,
    budget_exceeded: Cell<bool>,
    invalid_measure: Cell<bool>,
    measure_budget: usize,
}

impl<'a> SolverAdapter<'a> {
    fn new(
        tree: &'a mut RuntimeTree,
        measure: &'a mut dyn AuiIntrinsicMeasure,
        measure_budget: usize,
    ) -> Self {
        Self {
            tree,
            measure,
            measure_calls: Cell::new(0),
            visited_nodes: Cell::new(0),
            cache_hits: Cell::new(0),
            budget_exceeded: Cell::new(false),
            invalid_measure: Cell::new(false),
            measure_budget,
        }
    }

    fn stats(&self) -> AuiLayoutStats {
        AuiLayoutStats {
            clean_snapshot_reused: false,
            visited_nodes: self.visited_nodes.get(),
            measure_calls: self.measure_calls.get(),
            cache_hits: self.cache_hits.get(),
            ..Default::default()
        }
    }
}

impl TraversePartialTree for SolverAdapter<'_> {
    type ChildIter<'a>
        = std::iter::Map<std::iter::Copied<std::slice::Iter<'a, usize>>, fn(usize) -> NodeId>
    where
        Self: 'a;

    fn child_ids(&self, parent: NodeId) -> Self::ChildIter<'_> {
        fn to_node_id(value: usize) -> NodeId {
            NodeId::from(value)
        }
        self.tree.nodes[usize::from(parent)]
            .children
            .iter()
            .copied()
            .map(to_node_id)
    }

    fn child_count(&self, parent: NodeId) -> usize {
        self.tree.nodes[usize::from(parent)].children.len()
    }

    fn get_child_id(&self, parent: NodeId, index: usize) -> NodeId {
        NodeId::from(self.tree.nodes[usize::from(parent)].children[index])
    }
}

impl LayoutPartialTree for SolverAdapter<'_> {
    type CustomIdent = String;
    type CoreContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;

    fn get_core_container_style(&self, node: NodeId) -> &Style {
        &self.tree.nodes[usize::from(node)].style
    }

    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        let slot = usize::from(node);
        let old = &self.tree.nodes[slot].layout;
        // Content extents/baselines are not geometry inputs below. Record actual
        // local rectangle changes, including siblings moved by a parent's reflow.
        if old.location != layout.location || old.size != layout.size {
            self.tree.geometry_dirty.insert(slot);
        }
        self.tree.nodes[slot].layout = *layout;
    }

    fn resolve_calc_value(&self, _: *const (), _: f32) -> f32 {
        0.0
    }

    fn compute_child_layout(&mut self, node: NodeId, input: LayoutInput) -> LayoutOutput {
        self.visited_nodes.set(self.visited_nodes.get() + 1);
        compute_cached_layout(self, node, input, |tree, node, input| {
            if input.run_mode == RunMode::PerformHiddenLayout {
                return compute_hidden_layout(tree, node);
            }
            let index = usize::from(node);
            let style = tree.tree.nodes[index].style.clone();
            if style.display == Display::None {
                return compute_hidden_layout(tree, node);
            }
            if !tree.tree.nodes[index].children.is_empty() {
                let output = if style.display == Display::Grid {
                    compute_grid_layout(tree, node, input)
                } else {
                    compute_flexbox_layout(tree, node, input)
                };
                let source_id = tree.tree.nodes[index].source.node_id.clone();
                let child_ids = tree.tree.nodes[index]
                    .children
                    .iter()
                    .map(|&child| tree.tree.nodes[child].source.node_id.clone())
                    .collect::<Vec<_>>();
                if let Some(custom) = tree.measure.layout_children(
                    &source_id,
                    [output.size.width, output.size.height],
                    &child_ids,
                ) {
                    for child in custom {
                        if let Some(&slot) = tree.tree.nodes[index]
                            .children
                            .iter()
                            .find(|&&slot| tree.tree.nodes[slot].source.node_id == child.node_id)
                        {
                            let old = tree.tree.nodes[slot].layout;
                            tree.tree.nodes[slot].layout.location = Point {
                                x: child.rect.x,
                                y: child.rect.y,
                            };
                            tree.tree.nodes[slot].layout.size = Size {
                                width: child.rect.width,
                                height: child.rect.height,
                            };
                            if old.location != tree.tree.nodes[slot].layout.location
                                || old.size != tree.tree.nodes[slot].layout.size
                            {
                                tree.tree.geometry_dirty.insert(slot);
                            }
                        }
                    }
                }
                return output;
            }
            let calls = tree.measure_calls.get();
            if calls >= tree.measure_budget {
                tree.budget_exceeded.set(true);
                return LayoutOutput::HIDDEN;
            }
            tree.measure_calls.set(calls + 1);
            let source = &tree.tree.nodes[index].source;
            let measured = tree.measure.measure(AuiMeasureInput {
                node_id: &source.node_id,
                text: source.text.as_deref(),
                known_width: input.known_dimensions.width,
                known_height: input.known_dimensions.height,
                available_width: available(input.available_space.width),
                available_height: available(input.available_space.height),
            });
            if !measured.width.is_finite()
                || measured.width < 0.0
                || !measured.height.is_finite()
                || measured.height < 0.0
                || measured
                    .first_baseline
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
            {
                tree.invalid_measure.set(true);
                return LayoutOutput::HIDDEN;
            }
            let mut output = compute_leaf_layout(
                input,
                &style,
                |_, _| 0.0,
                |known, _| Size {
                    width: known.width.unwrap_or(measured.width),
                    height: known.height.unwrap_or(measured.height),
                },
            );
            output.first_baselines = Point {
                x: None,
                y: measured.first_baseline,
            };
            output
        })
    }
}

impl CacheTree for SolverAdapter<'_> {
    fn cache_get(&self, node: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        let result = self.tree.nodes[usize::from(node)].cache.get(input);
        if result.is_some() {
            self.cache_hits.set(self.cache_hits.get() + 1);
        }
        result
    }

    fn cache_store(&mut self, node: NodeId, input: &LayoutInput, output: LayoutOutput) {
        self.tree.nodes[usize::from(node)]
            .cache
            .store(input, output);
    }

    fn cache_clear(&mut self, node: NodeId) {
        self.tree.nodes[usize::from(node)].cache.clear();
    }
}

impl LayoutFlexboxContainer for SolverAdapter<'_> {
    type FlexboxContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    fn get_flexbox_container_style(&self, node: NodeId) -> &Style {
        &self.tree.nodes[usize::from(node)].style
    }

    fn get_flexbox_child_style(&self, node: NodeId) -> &Style {
        &self.tree.nodes[usize::from(node)].style
    }
}

impl LayoutGridContainer for SolverAdapter<'_> {
    type GridContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    fn get_grid_container_style(&self, node: NodeId) -> &Style {
        &self.tree.nodes[usize::from(node)].style
    }

    fn get_grid_child_style(&self, node: NodeId) -> &Style {
        &self.tree.nodes[usize::from(node)].style
    }
}

fn available(value: AvailableSpace) -> AuiAvailableSpace {
    match value {
        AvailableSpace::Definite(value) => AuiAvailableSpace::Definite(value),
        AvailableSpace::MinContent => AuiAvailableSpace::MinContent,
        AvailableSpace::MaxContent => AuiAvailableSpace::MaxContent,
    }
}

fn invalidate_to_root(tree: &mut RuntimeTree, mut index: usize) {
    loop {
        tree.nodes[index].cache.clear();
        let Some(parent) = tree.nodes[index].parent else {
            break;
        };
        index = parent;
    }
}

/// Stable mounted-tree slots in the snapshot's parent-before-child order.
/// One current mapping, never a history of revisions.
struct SnapshotSlots {
    tree_to_snapshot: Vec<Option<usize>>,
    snapshot_to_tree: Vec<usize>,
    overflow_slots: Vec<usize>,
}

impl SnapshotSlots {
    fn new(tree: &RuntimeTree, snapshot: &AuiLayoutSnapshot) -> Self {
        let ids: HashMap<_, _> = tree
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.source.node_id.as_str(), slot))
            .collect();
        let snapshot_to_tree: Vec<_> = snapshot
            .nodes
            .iter()
            .map(|node| ids[node.node_id.as_str()])
            .collect();
        let mut tree_to_snapshot = vec![None; tree.nodes.len()];
        for (snapshot_slot, &tree_slot) in snapshot_to_tree.iter().enumerate() {
            tree_to_snapshot[tree_slot] = Some(snapshot_slot);
        }
        let overflow_slots = snapshot
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(slot, node)| node.overflows_parent.then_some(slot))
            .collect();
        Self {
            tree_to_snapshot,
            snapshot_to_tree,
            overflow_slots,
        }
    }

    fn update(
        &mut self,
        tree: &RuntimeTree,
        snapshot: &mut AuiLayoutSnapshot,
        properties: &HashSet<usize>,
        root_rect: AuiComputedRect,
    ) -> (Vec<usize>, usize, usize) {
        let mut pending: BTreeSet<_> = tree
            .geometry_dirty
            .iter()
            .chain(properties)
            .filter_map(|&slot| self.tree_to_snapshot[slot])
            .collect();
        let mut updated = Vec::new();
        let mut overflow_summary_entries_updated = 0;
        while let Some(slot) = pending.pop_first() {
            let tree_slot = self.snapshot_to_tree[slot];
            let node = &tree.nodes[tree_slot];
            let is_root = tree_slot == tree.root;
            let parent_rect = if is_root {
                root_rect
            } else {
                snapshot.nodes[self.tree_to_snapshot[node.parent.expect("mounted parent")]
                    .expect("reachable parent")]
                .rect
            };
            let geometry = node_geometry(node, parent_rect, is_root);
            let old = &snapshot.nodes[slot];
            if old.overflows_parent != geometry.overflows_parent {
                overflow_summary_entries_updated += 1;
                if geometry.overflows_parent {
                    let position = self
                        .overflow_slots
                        .binary_search(&slot)
                        .expect_err("new overflow slot must not already be indexed");
                    self.overflow_slots.insert(position, slot);
                    snapshot
                        .overflow_node_ids
                        .insert(position, geometry.node_id.clone());
                } else {
                    let position = self
                        .overflow_slots
                        .binary_search(&slot)
                        .expect("cleared overflow slot must be indexed");
                    self.overflow_slots.remove(position);
                    snapshot.overflow_node_ids.remove(position);
                }
            }
            if old.rect != geometry.rect {
                // A moved/resized parent changes children's world coordinates,
                // manual anchors and overflow even when Taffy reused local layout.
                pending.extend(
                    node.children
                        .iter()
                        .filter_map(|&child| self.tree_to_snapshot[child]),
                );
            }
            snapshot.nodes[slot] = geometry;
            updated.push(slot);
        }
        (updated, 0, overflow_summary_entries_updated)
    }
}

fn build_snapshot(
    tree: &RuntimeTree,
    canvas_id: &str,
    revision: u64,
    root_rect: AuiComputedRect,
) -> AuiLayoutSnapshot {
    let mut snapshot = AuiLayoutSnapshot {
        revision,
        canvas_id: canvas_id.to_owned(),
        ..Default::default()
    };
    collect_geometry(tree, tree.root, root_rect, true, &mut snapshot);
    snapshot
}

fn collect_geometry(
    tree: &RuntimeTree,
    index: usize,
    parent_rect: AuiComputedRect,
    is_root: bool,
    snapshot: &mut AuiLayoutSnapshot,
) {
    let node = &tree.nodes[index];
    let geometry = node_geometry(node, parent_rect, is_root);
    let rect = geometry.rect;
    if geometry.overflows_parent {
        snapshot.overflow_node_ids.push(geometry.node_id.clone());
    }
    snapshot.nodes.push(geometry);
    for &child in &node.children {
        collect_geometry(tree, child, rect, false, snapshot);
    }
}

fn node_geometry(
    node: &RuntimeNode,
    parent_rect: AuiComputedRect,
    is_root: bool,
) -> AuiLayoutGeometry {
    let visibility = node.source.layout_visibility.unwrap_or_default();
    let rect = if visibility == LayoutVisibility::Collapsed {
        AuiComputedRect {
            x: parent_rect.x,
            y: parent_rect.y,
            width: 0.0,
            height: 0.0,
        }
    } else if is_root {
        parent_rect
    } else if node.manual_position {
        node.source.rect.resolve(parent_rect)
    } else {
        AuiComputedRect {
            x: parent_rect.x + node.layout.location.x,
            y: parent_rect.y + node.layout.location.y,
            width: node.layout.size.width,
            height: node.layout.size.height,
        }
    };
    let overflows_parent = !is_root
        && (rect.x < parent_rect.x - f32::EPSILON
            || rect.y < parent_rect.y - f32::EPSILON
            || rect.x + rect.width > parent_rect.x + parent_rect.width + f32::EPSILON
            || rect.y + rect.height > parent_rect.y + parent_rect.height + f32::EPSILON);
    AuiLayoutGeometry {
        node_id: node.source.node_id.clone(),
        rect,
        visibility,
        overflows_parent,
    }
}
