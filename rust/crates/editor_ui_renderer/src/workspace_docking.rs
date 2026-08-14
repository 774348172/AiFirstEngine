use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

use serde::{Deserialize, Serialize};

use crate::{native_editor_panel_manifest, UiPoint, UiRect};

pub const EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION: &str = "editor-workspace-layout.v1";
pub const EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION: &str = "editor-workspace-layout.v2";

macro_rules! workspace_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Option<Self> {
                let value = value.into();
                (!value.trim().is_empty()).then_some(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }
    };
}

workspace_id!(LayoutNodeId);
workspace_id!(PanelId);
workspace_id!(WorkspaceWindowId);

impl WorkspaceWindowId {
    pub fn main() -> Self {
        Self("main".to_string())
    }

    pub fn is_main(&self) -> bool {
        self.as_str() == "main"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceWindowPlacement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub display_id: Option<String>,
}

impl Default for WorkspaceWindowPlacement {
    fn default() -> Self {
        Self {
            x: 120.0,
            y: 80.0,
            width: 640.0,
            height: 480.0,
            display_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceWindowRoot {
    pub window_id: WorkspaceWindowId,
    pub root: DockNode,
    pub placement: WorkspaceWindowPlacement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTopology {
    pub schema_version: String,
    pub main_root: EditorWorkspaceLayout,
    pub floating_roots: Vec<WorkspaceWindowRoot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceDisplay {
    pub display_id: String,
    pub work_area: UiRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceWindowPlanEntry {
    pub window_id: WorkspaceWindowId,
    pub root: DockNode,
    pub placement: WorkspaceWindowPlacement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceWindowPlan {
    pub windows: Vec<WorkspaceWindowPlanEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanelSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelDescriptor {
    pub panel_id: PanelId,
    pub title: String,
    pub minimum_size: PanelSize,
    pub preferred_size: PanelSize,
    pub closable: bool,
    pub default_stack_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockSplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DockNode {
    Split {
        node_id: LayoutNodeId,
        axis: DockSplitAxis,
        ratio: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
    Stack {
        node_id: LayoutNodeId,
        tabs: Vec<PanelId>,
        active_panel_id: PanelId,
    },
}

impl DockNode {
    pub fn node_id(&self) -> &LayoutNodeId {
        match self {
            Self::Split { node_id, .. } | Self::Stack { node_id, .. } => node_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorWorkspaceLayout {
    pub schema_version: String,
    pub root: DockNode,
    pub closed_panels: Vec<PanelId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceLayoutDiagnostic {
    pub code: String,
    pub node_id: Option<LayoutNodeId>,
    pub panel_id: Option<PanelId>,
}

impl WorkspaceLayoutDiagnostic {
    fn new(
        code: &str,
        node_id: Option<&LayoutNodeId>,
        panel_id: Option<&PanelId>,
    ) -> WorkspaceLayoutDiagnostic {
        Self {
            code: code.to_string(),
            node_id: node_id.cloned(),
            panel_id: panel_id.cloned(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PanelRegistry {
    panels: BTreeMap<PanelId, PanelDescriptor>,
}

impl PanelRegistry {
    pub fn standard_editor() -> Self {
        let mut registry = Self::default();
        for entry in native_editor_panel_manifest()
            .iter()
            .filter(|entry| entry.dockable)
        {
            registry.register(default_descriptor(entry.panel_id));
        }
        registry
    }

    pub fn register(&mut self, descriptor: PanelDescriptor) -> bool {
        self.panels
            .insert(descriptor.panel_id.clone(), descriptor)
            .is_none()
    }

    pub fn get(&self, panel_id: &str) -> Option<&PanelDescriptor> {
        self.panels.get(panel_id)
    }

    pub fn contains(&self, panel_id: &str) -> bool {
        self.panels.contains_key(panel_id)
    }

    pub fn panel_ids(&self) -> impl Iterator<Item = &PanelId> {
        self.panels.keys()
    }

    pub fn len(&self) -> usize {
        self.panels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSplitter {
    pub node_id: LayoutNodeId,
    pub axis: DockSplitAxis,
    pub hit_rect: UiRect,
    pub visual_rect: UiRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceDragPreview {
    pub target_node_id: LayoutNodeId,
    pub zone: DockDropZone,
    pub rect: UiRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceDragWindowFacts {
    pub window_id: WorkspaceWindowId,
    pub screen_rect: UiRect,
    pub workspace_rect: UiRect,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceResolvedDockTargetToken {
    pub window_id: WorkspaceWindowId,
    pub node_id: LayoutNodeId,
    pub zone: DockDropZone,
    pub rect: UiRect,
    pub layout_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockDropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub layout_revision: u64,
    pub root: DockNode,
    pub node_rects: BTreeMap<LayoutNodeId, UiRect>,
    pub panel_rects: BTreeMap<PanelId, UiRect>,
    pub active_tabs: BTreeMap<LayoutNodeId, PanelId>,
    pub panel_descriptors: Vec<PanelDescriptor>,
    pub inspector_lock_available: bool,
    pub inspector_locked: bool,
    pub splitters: Vec<WorkspaceSplitter>,
    pub drag_preview: Option<WorkspaceDragPreview>,
    pub diagnostics: Vec<WorkspaceLayoutDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRestore {
    pub used_default: bool,
    pub diagnostics: Vec<WorkspaceLayoutDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceIntent {
    ResetLayout,
    ActivatePanel {
        panel_id: PanelId,
    },
    BeginSplitterResize {
        node_id: LayoutNodeId,
        pointer: UiPoint,
        workspace_rect: UiRect,
    },
    UpdateSplitterResize {
        pointer: UiPoint,
    },
    CommitSplitterResize,
    CancelSplitterResize,
    BeginPanelDrag {
        panel_id: PanelId,
        pointer: UiPoint,
        workspace_rect: UiRect,
    },
    BeginPanelDragInWindow {
        panel_id: PanelId,
        source_window_id: WorkspaceWindowId,
        pointer: UiPoint,
        workspace_rect: UiRect,
    },
    UpdatePanelDrag {
        pointer: UiPoint,
        workspace_rect: UiRect,
    },
    UpdatePanelDragAcrossWindows {
        screen_pointer: UiPoint,
        windows: Vec<WorkspaceDragWindowFacts>,
    },
    CommitPanelDrag,
    CommitPanelDragToFloating {
        window_id: WorkspaceWindowId,
        placement: WorkspaceWindowPlacement,
    },
    CancelPanelDrag,
    ClosePanel {
        panel_id: PanelId,
    },
    ShowPanel {
        panel_id: PanelId,
    },
    FloatPanel {
        panel_id: PanelId,
        window_id: WorkspaceWindowId,
        placement: WorkspaceWindowPlacement,
    },
    DockPanelToWindow {
        panel_id: PanelId,
        window_id: WorkspaceWindowId,
        target_stack_id: LayoutNodeId,
        zone: DockDropZone,
    },
    CloseFloatingWindow {
        window_id: WorkspaceWindowId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUpdate {
    pub changed: bool,
    pub layout_revision: u64,
    pub diagnostics: Vec<WorkspaceLayoutDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct EditorWorkspaceDockingModule {
    registry: PanelRegistry,
    layout: EditorWorkspaceLayout,
    layout_revision: u64,
    active_resize: Option<ActiveSplitterResize>,
    active_panel_drag: Option<ActivePanelDrag>,
    inspector_lock_available: bool,
    inspector_locked: bool,
    floating_roots: BTreeMap<WorkspaceWindowId, WorkspaceWindowRoot>,
}

#[derive(Debug, Clone)]
struct ActiveSplitterResize {
    node_id: LayoutNodeId,
    axis: DockSplitAxis,
    start_pointer_axis: f32,
    start_ratio: f32,
    axis_span: f32,
    minimum_ratio: f32,
    maximum_ratio: f32,
}

#[derive(Debug, Clone)]
struct ActivePanelDrag {
    source_panel_id: PanelId,
    source_stack_id: LayoutNodeId,
    start_pointer: UiPoint,
    original_layout: EditorWorkspaceLayout,
    original_floating_roots: BTreeMap<WorkspaceWindowId, WorkspaceWindowRoot>,
    source_window_id: WorkspaceWindowId,
    dragging: bool,
    target: Option<WorkspaceResolvedDockTargetToken>,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedDockTarget {
    node_id: LayoutNodeId,
    zone: DockDropZone,
    rect: UiRect,
}

const PANEL_DRAG_THRESHOLD: f32 = 6.0;

impl Default for EditorWorkspaceDockingModule {
    fn default() -> Self {
        Self::standard_editor()
    }
}

impl EditorWorkspaceDockingModule {
    pub fn standard_editor() -> Self {
        let registry = PanelRegistry::standard_editor();
        let layout = default_workspace_layout(&registry);
        Self {
            registry,
            layout,
            layout_revision: 1,
            active_resize: None,
            active_panel_drag: None,
            inspector_lock_available: false,
            inspector_locked: false,
            floating_roots: BTreeMap::new(),
        }
    }

    pub fn restore_or_default(
        registry: PanelRegistry,
        candidate: Option<EditorWorkspaceLayout>,
    ) -> (Self, WorkspaceRestore) {
        if let Some(layout) = candidate {
            match reconcile_workspace_layout(layout, &registry) {
                Ok((layout, diagnostics)) => {
                    return (
                        Self {
                            registry,
                            layout,
                            layout_revision: 1,
                            active_resize: None,
                            active_panel_drag: None,
                            inspector_lock_available: false,
                            inspector_locked: false,
                            floating_roots: BTreeMap::new(),
                        },
                        WorkspaceRestore {
                            used_default: false,
                            diagnostics,
                        },
                    );
                }
                Err(diagnostics) => {
                    let layout = default_workspace_layout(&registry);
                    return (
                        Self {
                            registry,
                            layout,
                            layout_revision: 1,
                            active_resize: None,
                            active_panel_drag: None,
                            inspector_lock_available: false,
                            inspector_locked: false,
                            floating_roots: BTreeMap::new(),
                        },
                        WorkspaceRestore {
                            used_default: true,
                            diagnostics,
                        },
                    );
                }
            }
        }
        let layout = default_workspace_layout(&registry);
        (
            Self {
                registry,
                layout,
                layout_revision: 1,
                active_resize: None,
                active_panel_drag: None,
                inspector_lock_available: false,
                inspector_locked: false,
                floating_roots: BTreeMap::new(),
            },
            WorkspaceRestore {
                used_default: true,
                diagnostics: vec![WorkspaceLayoutDiagnostic::new(
                    "layout_restore_default",
                    None,
                    None,
                )],
            },
        )
    }

    pub fn restore_topology_or_default(
        registry: PanelRegistry,
        candidate: Option<WorkspaceTopology>,
        legacy: Option<EditorWorkspaceLayout>,
    ) -> (Self, WorkspaceRestore) {
        if let Some(topology) = candidate {
            if topology.schema_version == EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION {
                let mut floating_claims = BTreeSet::new();
                for floating in &topology.floating_roots {
                    collect_panels_into_set(&floating.root, &mut floating_claims);
                }
                if let Ok((layout, mut diagnostics)) =
                    reconcile_workspace_layout(topology.main_root, &registry)
                {
                    let mut layout = layout;
                    for panel_id in &floating_claims {
                        if stack_id_containing(&layout.root, panel_id).is_some() {
                            if let Some(root) = remove_panel(layout.root.clone(), panel_id) {
                                layout.root = root;
                            }
                        }
                    }
                    let mut floating_roots = BTreeMap::new();
                    for floating in topology.floating_roots {
                        let mut candidate_roots =
                            floating_roots.values().cloned().collect::<Vec<_>>();
                        candidate_roots.push(floating.clone());
                        let candidate_topology = WorkspaceTopology {
                            schema_version: EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION.to_string(),
                            main_root: layout.clone(),
                            floating_roots: candidate_roots,
                        };
                        if floating.window_id.is_main()
                            || floating_roots.contains_key(&floating.window_id)
                            || validate_floating_root(&floating, &registry).is_err()
                            || validate_workspace_topology(&candidate_topology, &registry).is_err()
                        {
                            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                                "discarded_invalid_floating_root",
                                Some(floating.root.node_id()),
                                None,
                            ));
                        } else {
                            floating_roots.insert(floating.window_id.clone(), floating);
                        }
                    }
                    let owned_panels = registry
                        .panel_ids()
                        .filter(|panel_id| {
                            topology_panel_location(&layout, &floating_roots, panel_id).is_none()
                                && !layout.closed_panels.contains(panel_id)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    for panel_id in owned_panels {
                        insert_panel_at_default(&mut layout.root, &panel_id, &registry);
                    }
                    let module = Self {
                        registry: registry.clone(),
                        layout,
                        layout_revision: 1,
                        active_resize: None,
                        active_panel_drag: None,
                        inspector_lock_available: false,
                        inspector_locked: false,
                        floating_roots,
                    };
                    if validate_workspace_topology(&module.topology(), &module.registry).is_ok() {
                        return (
                            module,
                            WorkspaceRestore {
                                used_default: false,
                                diagnostics,
                            },
                        );
                    }
                }
            }
        }
        if let Some(legacy) = legacy {
            let (module, mut restore) = Self::restore_or_default(registry, Some(legacy));
            if !restore.used_default {
                restore.diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "migrated_layout_v1_to_v2",
                    None,
                    None,
                ));
            }
            return (module, restore);
        }
        Self::restore_or_default(registry, None)
    }

    pub fn registry(&self) -> &PanelRegistry {
        &self.registry
    }

    pub fn layout(&self) -> &EditorWorkspaceLayout {
        &self.layout
    }

    pub fn topology(&self) -> WorkspaceTopology {
        WorkspaceTopology {
            schema_version: EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION.to_string(),
            main_root: self.layout.clone(),
            floating_roots: self.floating_roots.values().cloned().collect(),
        }
    }

    pub fn window_plan(&self, displays: &[WorkspaceDisplay]) -> WorkspaceWindowPlan {
        let main_placement = displays
            .first()
            .map(|display| placement_from_rect(display.work_area, Some(display.display_id.clone())))
            .unwrap_or_default();
        let mut windows = vec![WorkspaceWindowPlanEntry {
            window_id: WorkspaceWindowId::main(),
            root: self.layout.root.clone(),
            placement: main_placement,
        }];
        windows.extend(
            self.floating_roots
                .values()
                .map(|floating| WorkspaceWindowPlanEntry {
                    window_id: floating.window_id.clone(),
                    root: floating.root.clone(),
                    placement: clamp_placement(&floating.placement, displays),
                }),
        );
        WorkspaceWindowPlan { windows }
    }

    pub fn update(&mut self, intent: WorkspaceIntent) -> WorkspaceUpdate {
        match intent {
            WorkspaceIntent::ResetLayout => {
                self.layout = default_workspace_layout(&self.registry);
                self.floating_roots.clear();
                self.active_resize = None;
                self.active_panel_drag = None;
                self.layout_revision = self.layout_revision.saturating_add(1);
                WorkspaceUpdate {
                    changed: true,
                    layout_revision: self.layout_revision,
                    diagnostics: Vec::new(),
                }
            }
            WorkspaceIntent::ActivatePanel { panel_id } => {
                let Some(active_panel_id) =
                    stack_active_panel_containing_mut(&mut self.layout.root, &panel_id)
                else {
                    return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                        "panel_not_in_workspace",
                        None,
                        Some(&panel_id),
                    ));
                };
                let changed = *active_panel_id != panel_id;
                if changed {
                    *active_panel_id = panel_id;
                    self.layout_revision = self.layout_revision.saturating_add(1);
                }
                WorkspaceUpdate {
                    changed,
                    layout_revision: self.layout_revision,
                    diagnostics: Vec::new(),
                }
            }
            WorkspaceIntent::BeginSplitterResize {
                node_id,
                pointer,
                workspace_rect,
            } => self.begin_splitter_resize(node_id, pointer, workspace_rect),
            WorkspaceIntent::UpdateSplitterResize { pointer } => {
                self.update_splitter_resize(pointer)
            }
            WorkspaceIntent::CommitSplitterResize => {
                if self.active_resize.take().is_none() {
                    return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                        "splitter_resize_not_active",
                        None,
                        None,
                    ));
                }
                WorkspaceUpdate {
                    changed: false,
                    layout_revision: self.layout_revision,
                    diagnostics: Vec::new(),
                }
            }
            WorkspaceIntent::CancelSplitterResize => self.cancel_splitter_resize(),
            WorkspaceIntent::BeginPanelDrag {
                panel_id,
                pointer,
                workspace_rect,
            } => self.begin_panel_drag_in_window(
                panel_id,
                WorkspaceWindowId::main(),
                pointer,
                workspace_rect,
            ),
            WorkspaceIntent::BeginPanelDragInWindow {
                panel_id,
                source_window_id,
                pointer,
                workspace_rect,
            } => {
                self.begin_panel_drag_in_window(panel_id, source_window_id, pointer, workspace_rect)
            }
            WorkspaceIntent::UpdatePanelDrag {
                pointer,
                workspace_rect,
            } => self.update_panel_drag(pointer, workspace_rect),
            WorkspaceIntent::UpdatePanelDragAcrossWindows {
                screen_pointer,
                windows,
            } => self.update_panel_drag_across_windows(screen_pointer, &windows),
            WorkspaceIntent::CommitPanelDrag => self.commit_panel_drag(),
            WorkspaceIntent::CommitPanelDragToFloating {
                window_id,
                placement,
            } => self.commit_panel_drag_to_floating(window_id, placement),
            WorkspaceIntent::CancelPanelDrag => self.cancel_panel_drag(),
            WorkspaceIntent::ClosePanel { panel_id } => self.close_panel(panel_id),
            WorkspaceIntent::ShowPanel { panel_id } => self.show_panel(panel_id),
            WorkspaceIntent::FloatPanel {
                panel_id,
                window_id,
                placement,
            } => self.float_panel(panel_id, window_id, placement),
            WorkspaceIntent::DockPanelToWindow {
                panel_id,
                window_id,
                target_stack_id,
                zone,
            } => self.dock_panel_to_window(panel_id, window_id, target_stack_id, zone),
            WorkspaceIntent::CloseFloatingWindow { window_id } => {
                self.close_floating_window(window_id)
            }
        }
    }

    pub fn active_resize_node_id(&self) -> Option<&LayoutNodeId> {
        self.active_resize.as_ref().map(|resize| &resize.node_id)
    }

    pub fn active_panel_id(&self, stack_id: &str) -> Option<&PanelId> {
        active_panel_for_stack(&self.layout.root, stack_id)
    }

    pub fn active_panel_drag_id(&self) -> Option<&PanelId> {
        self.active_panel_drag
            .as_ref()
            .map(|drag| &drag.source_panel_id)
    }

    pub fn panel_drag_is_active(&self) -> bool {
        self.active_panel_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging)
    }

    pub fn drag_requires_native_proxy(&self) -> bool {
        self.active_panel_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging && drag.target.is_none())
    }

    pub fn resolved_drag_target_token(&self) -> Option<&WorkspaceResolvedDockTargetToken> {
        self.active_panel_drag
            .as_ref()
            .and_then(|drag| drag.target.as_ref())
    }

    pub fn set_inspector_lock_presentation(&mut self, available: bool, locked: bool) {
        self.inspector_lock_available = available;
        self.inspector_locked = locked;
    }

    pub fn snapshot(&self, rect: UiRect) -> WorkspaceSnapshot {
        self.snapshot_window(&WorkspaceWindowId::main(), rect)
            .expect("the main workspace root always exists")
    }

    pub fn snapshot_window(
        &self,
        window_id: &WorkspaceWindowId,
        rect: UiRect,
    ) -> Option<WorkspaceSnapshot> {
        let rect = sanitize_rect(rect);
        let root = if window_id.is_main() {
            &self.layout.root
        } else {
            &self.floating_roots.get(window_id)?.root
        };
        let mut snapshot = WorkspaceSnapshot {
            layout_revision: self.layout_revision,
            root: root.clone(),
            node_rects: BTreeMap::new(),
            panel_rects: BTreeMap::new(),
            active_tabs: BTreeMap::new(),
            panel_descriptors: self.registry.panels.values().cloned().collect(),
            inspector_lock_available: self.inspector_lock_available,
            inspector_locked: self.inspector_locked,
            splitters: Vec::new(),
            drag_preview: self.active_panel_drag.as_ref().and_then(|drag| {
                drag.target.as_ref().map(|target| WorkspaceDragPreview {
                    target_node_id: target.node_id.clone(),
                    zone: target.zone,
                    rect: target.rect,
                })
            }),
            diagnostics: Vec::new(),
        };
        resolve_node(root, rect, &self.registry, &mut snapshot);
        Some(snapshot)
    }

    fn begin_splitter_resize(
        &mut self,
        node_id: LayoutNodeId,
        pointer: UiPoint,
        workspace_rect: UiRect,
    ) -> WorkspaceUpdate {
        let snapshot = self.snapshot(workspace_rect);
        let Some(node_rect) = snapshot.node_rects.get(&node_id).copied() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "splitter_node_not_found",
                Some(&node_id),
                None,
            ));
        };
        let Some((axis, ratio, first_minimum, second_minimum)) =
            split_resize_inputs(&self.layout.root, &node_id, &self.registry)
        else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "splitter_node_not_found",
                Some(&node_id),
                None,
            ));
        };
        let available = axis_extent(node_rect, axis).max(0.0);
        let first_minimum = axis_size(first_minimum, axis);
        let second_minimum = axis_size(second_minimum, axis);
        let (minimum_ratio, maximum_ratio) =
            if available > 0.0 && first_minimum + second_minimum <= available {
                (
                    (first_minimum / available).clamp(0.0, 1.0),
                    (1.0 - second_minimum / available).clamp(0.0, 1.0),
                )
            } else {
                (0.0, 1.0)
            };
        self.active_resize = Some(ActiveSplitterResize {
            node_id,
            axis,
            start_pointer_axis: pointer_axis(pointer, axis),
            start_ratio: ratio,
            axis_span: available.max(1.0),
            minimum_ratio,
            maximum_ratio,
        });
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn update_splitter_resize(&mut self, pointer: UiPoint) -> WorkspaceUpdate {
        let Some(resize) = self.active_resize.clone() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "splitter_resize_not_active",
                None,
                None,
            ));
        };
        let delta = pointer_axis(pointer, resize.axis) - resize.start_pointer_axis;
        let ratio = (resize.start_ratio + delta / resize.axis_span)
            .clamp(resize.minimum_ratio, resize.maximum_ratio);
        let changed = set_split_ratio(&mut self.layout.root, &resize.node_id, ratio);
        if changed {
            self.layout_revision = self.layout_revision.saturating_add(1);
        }
        WorkspaceUpdate {
            changed,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn cancel_splitter_resize(&mut self) -> WorkspaceUpdate {
        let Some(resize) = self.active_resize.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "splitter_resize_not_active",
                None,
                None,
            ));
        };
        let changed = set_split_ratio(&mut self.layout.root, &resize.node_id, resize.start_ratio);
        if changed {
            self.layout_revision = self.layout_revision.saturating_add(1);
        }
        WorkspaceUpdate {
            changed,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn begin_panel_drag_in_window(
        &mut self,
        panel_id: PanelId,
        source_window_id: WorkspaceWindowId,
        pointer: UiPoint,
        workspace_rect: UiRect,
    ) -> WorkspaceUpdate {
        if self.active_resize.is_some() || self.active_panel_drag.is_some() {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "workspace_pointer_owner_busy",
                None,
                Some(&panel_id),
            ));
        }
        let source_root = if source_window_id.is_main() {
            Some(&self.layout.root)
        } else {
            self.floating_roots
                .get(&source_window_id)
                .map(|window| &window.root)
        };
        let Some(source_stack_id) = source_root
            .and_then(|root| stack_id_containing(root, &panel_id))
            .cloned()
        else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_in_workspace",
                None,
                Some(&panel_id),
            ));
        };
        let Some(snapshot) = self.snapshot_window(&source_window_id, workspace_rect) else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "source_window_not_found",
                None,
                Some(&panel_id),
            ));
        };
        if !snapshot.node_rects.contains_key(&source_stack_id) {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "source_stack_not_found",
                Some(&source_stack_id),
                Some(&panel_id),
            ));
        }
        self.active_panel_drag = Some(ActivePanelDrag {
            source_panel_id: panel_id,
            source_stack_id,
            start_pointer: pointer,
            original_layout: self.layout.clone(),
            original_floating_roots: self.floating_roots.clone(),
            source_window_id,
            dragging: false,
            target: None,
        });
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn update_panel_drag(&mut self, pointer: UiPoint, workspace_rect: UiRect) -> WorkspaceUpdate {
        let Some(mut drag) = self.active_panel_drag.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_drag_not_active",
                None,
                None,
            ));
        };
        let delta_x = pointer.x - drag.start_pointer.x;
        let delta_y = pointer.y - drag.start_pointer.y;
        if !drag.dragging
            && delta_x * delta_x + delta_y * delta_y >= PANEL_DRAG_THRESHOLD * PANEL_DRAG_THRESHOLD
        {
            drag.dragging = true;
        }
        if drag.dragging {
            drag.target =
                resolve_dock_target(&self.layout, &self.registry, workspace_rect, pointer).map(
                    |target| WorkspaceResolvedDockTargetToken {
                        window_id: WorkspaceWindowId::main(),
                        node_id: target.node_id,
                        zone: target.zone,
                        rect: target.rect,
                        layout_revision: self.layout_revision,
                    },
                );
        }
        self.active_panel_drag = Some(drag);
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn update_panel_drag_across_windows(
        &mut self,
        screen_pointer: UiPoint,
        windows: &[WorkspaceDragWindowFacts],
    ) -> WorkspaceUpdate {
        let Some(mut drag) = self.active_panel_drag.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_drag_not_active",
                None,
                None,
            ));
        };
        let source_facts = windows
            .iter()
            .find(|facts| facts.window_id == drag.source_window_id);
        let source_pointer = source_facts
            .map(|facts| screen_to_workspace(screen_pointer, facts))
            .unwrap_or(screen_pointer);
        let delta_x = source_pointer.x - drag.start_pointer.x;
        let delta_y = source_pointer.y - drag.start_pointer.y;
        if !drag.dragging
            && delta_x * delta_x + delta_y * delta_y >= PANEL_DRAG_THRESHOLD * PANEL_DRAG_THRESHOLD
        {
            drag.dragging = true;
        }
        if drag.dragging {
            let matches = windows
                .iter()
                .filter(|facts| rect_contains(facts.screen_rect, screen_pointer))
                .collect::<Vec<_>>();
            drag.target = if matches.len() == 1 {
                let facts = matches[0];
                let local_pointer = screen_to_workspace(screen_pointer, facts);
                let root = if facts.window_id.is_main() {
                    Some(&self.layout.root)
                } else {
                    self.floating_roots
                        .get(&facts.window_id)
                        .map(|window| &window.root)
                };
                root.and_then(|root| {
                    let mut target_layout = self.layout.clone();
                    target_layout.root = root.clone();
                    resolve_dock_target(
                        &target_layout,
                        &self.registry,
                        facts.workspace_rect,
                        local_pointer,
                    )
                    .map(|target| WorkspaceResolvedDockTargetToken {
                        window_id: facts.window_id.clone(),
                        node_id: target.node_id,
                        zone: target.zone,
                        rect: target.rect,
                        layout_revision: self.layout_revision,
                    })
                })
            } else {
                None
            };
        }
        self.active_panel_drag = Some(drag);
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn commit_panel_drag(&mut self) -> WorkspaceUpdate {
        let Some(drag) = self.active_panel_drag.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_drag_not_active",
                None,
                None,
            ));
        };
        if !drag.dragging {
            let Some(active_panel_id) =
                stack_active_panel_containing_mut(&mut self.layout.root, &drag.source_panel_id)
            else {
                return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                    "stale_drag_source",
                    Some(&drag.source_stack_id),
                    Some(&drag.source_panel_id),
                ));
            };
            let changed = *active_panel_id != drag.source_panel_id;
            if changed {
                *active_panel_id = drag.source_panel_id;
                self.layout_revision = self.layout_revision.saturating_add(1);
            }
            return WorkspaceUpdate {
                changed,
                layout_revision: self.layout_revision,
                diagnostics: Vec::new(),
            };
        }
        let Some(target) = drag.target else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "dock_target_not_found",
                None,
                Some(&drag.source_panel_id),
            ));
        };
        if target.layout_revision != self.layout_revision {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "stale_dock_target",
                Some(&target.node_id),
                Some(&drag.source_panel_id),
            ));
        }
        self.dock_panel_to_window(
            drag.source_panel_id,
            target.window_id,
            target.node_id,
            target.zone,
        )
    }

    fn commit_panel_drag_to_floating(
        &mut self,
        window_id: WorkspaceWindowId,
        placement: WorkspaceWindowPlacement,
    ) -> WorkspaceUpdate {
        let Some(drag) = self.active_panel_drag.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_drag_not_active",
                None,
                None,
            ));
        };
        if !drag.dragging || drag.target.is_some() {
            self.active_panel_drag = Some(drag);
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "floating_commit_not_eligible",
                None,
                None,
            ));
        }
        self.float_panel(drag.source_panel_id, window_id, placement)
    }

    fn cancel_panel_drag(&mut self) -> WorkspaceUpdate {
        let Some(drag) = self.active_panel_drag.take() else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_drag_not_active",
                None,
                None,
            ));
        };
        self.layout = drag.original_layout;
        self.floating_roots = drag.original_floating_roots;
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn close_panel(&mut self, panel_id: PanelId) -> WorkspaceUpdate {
        let Some(descriptor) = self.registry.get(panel_id.as_str()) else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "unknown_panel",
                None,
                Some(&panel_id),
            ));
        };
        if !descriptor.closable {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_closable",
                None,
                Some(&panel_id),
            ));
        }
        if stack_id_containing(&self.layout.root, &panel_id).is_none() {
            if let Some(window_id) =
                floating_window_containing(&self.floating_roots, &panel_id).cloned()
            {
                let root = self.floating_roots.get(&window_id).unwrap().root.clone();
                match remove_panel(root, &panel_id) {
                    Some(root) => self.floating_roots.get_mut(&window_id).unwrap().root = root,
                    None => {
                        self.floating_roots.remove(&window_id);
                    }
                }
                if !self.layout.closed_panels.contains(&panel_id) {
                    self.layout.closed_panels.push(panel_id);
                    self.layout.closed_panels.sort();
                }
                self.active_panel_drag = None;
                self.active_resize = None;
                self.layout_revision = self.layout_revision.saturating_add(1);
                return WorkspaceUpdate {
                    changed: true,
                    layout_revision: self.layout_revision,
                    diagnostics: Vec::new(),
                };
            }
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_in_workspace",
                None,
                Some(&panel_id),
            ));
        }
        let Some(root) = remove_panel(self.layout.root.clone(), &panel_id) else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_close_would_empty_workspace",
                None,
                Some(&panel_id),
            ));
        };
        self.layout.root = root;
        if !self.layout.closed_panels.contains(&panel_id) {
            self.layout.closed_panels.push(panel_id);
            self.layout.closed_panels.sort();
        }
        self.active_panel_drag = None;
        self.active_resize = None;
        self.layout_revision = self.layout_revision.saturating_add(1);
        WorkspaceUpdate {
            changed: true,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn show_panel(&mut self, panel_id: PanelId) -> WorkspaceUpdate {
        let Some(descriptor) = self.registry.get(panel_id.as_str()) else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "unknown_panel",
                None,
                Some(&panel_id),
            ));
        };
        if topology_panel_location(&self.layout, &self.floating_roots, &panel_id).is_some() {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_already_visible",
                None,
                Some(&panel_id),
            ));
        }
        if !self.layout.closed_panels.contains(&panel_id) {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_closed",
                None,
                Some(&panel_id),
            ));
        }
        let preferred_stack_id = layout_node_id(&format!(
            "workspace/{}",
            descriptor.default_stack_id.as_str()
        ));
        if !insert_panel_at_default(&mut self.layout.root, &panel_id, &self.registry) {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_default_stack_missing",
                Some(&preferred_stack_id),
                Some(&panel_id),
            ));
        }
        self.layout
            .closed_panels
            .retain(|closed| closed != &panel_id);
        self.layout_revision = self.layout_revision.saturating_add(1);
        WorkspaceUpdate {
            changed: true,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn float_panel(
        &mut self,
        panel_id: PanelId,
        window_id: WorkspaceWindowId,
        placement: WorkspaceWindowPlacement,
    ) -> WorkspaceUpdate {
        if window_id.is_main() || self.floating_roots.contains_key(&window_id) {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "workspace_window_id_conflict",
                None,
                Some(&panel_id),
            ));
        }
        if self.registry.get(panel_id.as_str()).is_none()
            || topology_panel_location(&self.layout, &self.floating_roots, &panel_id).is_none()
        {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_in_workspace",
                None,
                Some(&panel_id),
            ));
        }
        let mut main = self.layout.clone();
        let mut floating = self.floating_roots.clone();
        if stack_id_containing(&main.root, &panel_id).is_some() {
            let Some(root) = remove_panel(main.root.clone(), &panel_id) else {
                return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                    "panel_float_would_empty_main",
                    None,
                    Some(&panel_id),
                ));
            };
            main.root = root;
        } else if let Some(source_window_id) =
            floating_window_containing(&floating, &panel_id).cloned()
        {
            let source = floating.get(&source_window_id).unwrap();
            match remove_panel(source.root.clone(), &panel_id) {
                Some(root) => floating.get_mut(&source_window_id).unwrap().root = root,
                None => {
                    floating.remove(&source_window_id);
                }
            }
        }
        let root = WorkspaceWindowRoot {
            window_id: window_id.clone(),
            root: DockNode::Stack {
                node_id: layout_node_id(&format!("workspace/{}/root", window_id.as_str())),
                tabs: vec![panel_id.clone()],
                active_panel_id: panel_id,
            },
            placement: sanitize_placement(placement),
        };
        floating.insert(window_id, root);
        let candidate = WorkspaceTopology {
            schema_version: EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION.to_string(),
            main_root: main,
            floating_roots: floating.values().cloned().collect(),
        };
        if let Err(diagnostics) = validate_workspace_topology(&candidate, &self.registry) {
            return WorkspaceUpdate {
                changed: false,
                layout_revision: self.layout_revision,
                diagnostics,
            };
        }
        self.layout = candidate.main_root;
        self.floating_roots = candidate
            .floating_roots
            .into_iter()
            .map(|root| (root.window_id.clone(), root))
            .collect();
        self.layout_revision = self.layout_revision.saturating_add(1);
        WorkspaceUpdate {
            changed: true,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn dock_panel_to_window(
        &mut self,
        panel_id: PanelId,
        window_id: WorkspaceWindowId,
        target_stack_id: LayoutNodeId,
        zone: DockDropZone,
    ) -> WorkspaceUpdate {
        let target_exists = if window_id.is_main() {
            is_stack(&self.layout.root, &target_stack_id)
        } else {
            self.floating_roots
                .get(&window_id)
                .is_some_and(|root| is_stack(&root.root, &target_stack_id))
        };
        if !target_exists {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "stale_dock_target",
                Some(&target_stack_id),
                Some(&panel_id),
            ));
        }
        let mut main = self.layout.clone();
        let mut floating = self.floating_roots.clone();
        let source_window = floating_window_containing(&floating, &panel_id).cloned();
        if stack_id_containing(&main.root, &panel_id).is_some() {
            let Some(root) = remove_panel(main.root.clone(), &panel_id) else {
                return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                    "panel_move_would_empty_main",
                    None,
                    Some(&panel_id),
                ));
            };
            main.root = root;
        } else if let Some(source_window) = &source_window {
            let source = floating.get(source_window).unwrap();
            match remove_panel(source.root.clone(), &panel_id) {
                Some(root) => floating.get_mut(source_window).unwrap().root = root,
                None => {
                    floating.remove(source_window);
                }
            }
        } else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "panel_not_in_workspace",
                None,
                Some(&panel_id),
            ));
        }
        let target_root = if window_id.is_main() {
            &mut main.root
        } else {
            &mut floating.get_mut(&window_id).unwrap().root
        };
        let inserted = match zone {
            DockDropZone::Center => {
                insert_panel_into_stack(target_root, &target_stack_id, &panel_id)
            }
            _ => replace_stack_with_split(
                target_root,
                &target_stack_id,
                &panel_id,
                zone,
                self.layout_revision.saturating_add(1),
            ),
        };
        if !inserted {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "stale_dock_target",
                Some(&target_stack_id),
                Some(&panel_id),
            ));
        }
        let candidate = WorkspaceTopology {
            schema_version: EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION.to_string(),
            main_root: main,
            floating_roots: floating.values().cloned().collect(),
        };
        if let Err(diagnostics) = validate_workspace_topology(&candidate, &self.registry) {
            return WorkspaceUpdate {
                changed: false,
                layout_revision: self.layout_revision,
                diagnostics,
            };
        }
        self.layout = candidate.main_root;
        self.floating_roots = candidate
            .floating_roots
            .into_iter()
            .map(|root| (root.window_id.clone(), root))
            .collect();
        self.layout_revision = self.layout_revision.saturating_add(1);
        WorkspaceUpdate {
            changed: true,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn close_floating_window(&mut self, window_id: WorkspaceWindowId) -> WorkspaceUpdate {
        let Some(window) = self.floating_roots.get(&window_id) else {
            return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                "floating_window_not_found",
                None,
                None,
            ));
        };
        let mut panels = Vec::new();
        collect_panels(&window.root, &mut panels);
        let mut main = self.layout.clone();
        for panel_id in panels {
            if !insert_panel_at_default(&mut main.root, &panel_id, &self.registry) {
                return self.unchanged_update(WorkspaceLayoutDiagnostic::new(
                    "panel_default_stack_missing",
                    None,
                    Some(&panel_id),
                ));
            }
        }
        self.layout = main;
        self.floating_roots.remove(&window_id);
        self.layout_revision = self.layout_revision.saturating_add(1);
        WorkspaceUpdate {
            changed: true,
            layout_revision: self.layout_revision,
            diagnostics: Vec::new(),
        }
    }

    fn unchanged_update(&self, diagnostic: WorkspaceLayoutDiagnostic) -> WorkspaceUpdate {
        WorkspaceUpdate {
            changed: false,
            layout_revision: self.layout_revision,
            diagnostics: vec![diagnostic],
        }
    }
}

fn sanitize_rect(rect: UiRect) -> UiRect {
    UiRect {
        x: if rect.x.is_finite() { rect.x } else { 0.0 },
        y: if rect.y.is_finite() { rect.y } else { 0.0 },
        width: if rect.width.is_finite() {
            rect.width.max(0.0)
        } else {
            0.0
        },
        height: if rect.height.is_finite() {
            rect.height.max(0.0)
        } else {
            0.0
        },
    }
}

fn screen_to_workspace(screen_pointer: UiPoint, facts: &WorkspaceDragWindowFacts) -> UiPoint {
    let scale = if facts.scale_factor.is_finite() {
        facts.scale_factor.max(0.01)
    } else {
        1.0
    };
    UiPoint {
        x: facts.workspace_rect.x + (screen_pointer.x - facts.screen_rect.x) / scale,
        y: facts.workspace_rect.y + (screen_pointer.y - facts.screen_rect.y) / scale,
    }
}

fn stack_active_panel_containing_mut<'a>(
    node: &'a mut DockNode,
    panel_id: &PanelId,
) -> Option<&'a mut PanelId> {
    match node {
        DockNode::Split { first, second, .. } => {
            if let Some(active) = stack_active_panel_containing_mut(first, panel_id) {
                Some(active)
            } else {
                stack_active_panel_containing_mut(second, panel_id)
            }
        }
        DockNode::Stack {
            tabs,
            active_panel_id,
            ..
        } => tabs
            .iter()
            .any(|tab| tab == panel_id)
            .then_some(active_panel_id),
    }
}

fn active_panel_for_stack<'a>(node: &'a DockNode, stack_id: &str) -> Option<&'a PanelId> {
    match node {
        DockNode::Split { first, second, .. } => active_panel_for_stack(first, stack_id)
            .or_else(|| active_panel_for_stack(second, stack_id)),
        DockNode::Stack {
            node_id,
            active_panel_id,
            ..
        } => (node_id.as_str() == stack_id).then_some(active_panel_id),
    }
}

fn stack_id_containing<'a>(node: &'a DockNode, panel_id: &PanelId) -> Option<&'a LayoutNodeId> {
    match node {
        DockNode::Split { first, second, .. } => {
            stack_id_containing(first, panel_id).or_else(|| stack_id_containing(second, panel_id))
        }
        DockNode::Stack { node_id, tabs, .. } => {
            tabs.iter().any(|tab| tab == panel_id).then_some(node_id)
        }
    }
}

fn resolve_dock_target(
    layout: &EditorWorkspaceLayout,
    registry: &PanelRegistry,
    workspace_rect: UiRect,
    pointer: UiPoint,
) -> Option<ResolvedDockTarget> {
    let mut snapshot = WorkspaceSnapshot {
        layout_revision: 0,
        root: layout.root.clone(),
        node_rects: BTreeMap::new(),
        panel_rects: BTreeMap::new(),
        active_tabs: BTreeMap::new(),
        panel_descriptors: registry.panels.values().cloned().collect(),
        inspector_lock_available: false,
        inspector_locked: false,
        splitters: Vec::new(),
        drag_preview: None,
        diagnostics: Vec::new(),
    };
    resolve_node(
        &layout.root,
        sanitize_rect(workspace_rect),
        registry,
        &mut snapshot,
    );
    let mut stack_ids = Vec::new();
    collect_stack_ids(&layout.root, &mut stack_ids);
    let (node_id, rect) = stack_ids
        .into_iter()
        .filter_map(|node_id| {
            snapshot
                .node_rects
                .get(node_id)
                .copied()
                .filter(|rect| rect_contains(*rect, pointer))
                .map(|rect| (node_id.clone(), rect))
        })
        .min_by(|(_, left), (_, right)| {
            (left.width * left.height)
                .partial_cmp(&(right.width * right.height))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let zone = dock_zone(rect, pointer);
    Some(ResolvedDockTarget {
        node_id,
        zone,
        rect: dock_preview_rect(rect, zone),
    })
}

fn collect_stack_ids<'a>(node: &'a DockNode, output: &mut Vec<&'a LayoutNodeId>) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_stack_ids(first, output);
            collect_stack_ids(second, output);
        }
        DockNode::Stack { node_id, .. } => output.push(node_id),
    }
}

fn rect_contains(rect: UiRect, pointer: UiPoint) -> bool {
    pointer.x >= rect.x
        && pointer.y >= rect.y
        && pointer.x <= rect.x + rect.width
        && pointer.y <= rect.y + rect.height
}

fn dock_zone(rect: UiRect, pointer: UiPoint) -> DockDropZone {
    let x = if rect.width > 0.0 {
        ((pointer.x - rect.x) / rect.width).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let y = if rect.height > 0.0 {
        ((pointer.y - rect.y) / rect.height).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let candidates = [
        (x, DockDropZone::Left),
        (1.0 - x, DockDropZone::Right),
        (y, DockDropZone::Top),
        (1.0 - y, DockDropZone::Bottom),
    ];
    let (distance, zone) = candidates
        .into_iter()
        .min_by(|(left, _), (right, _)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("dock edge candidates");
    if distance <= 0.25 {
        zone
    } else {
        DockDropZone::Center
    }
}

fn dock_preview_rect(rect: UiRect, zone: DockDropZone) -> UiRect {
    match zone {
        DockDropZone::Center => rect,
        DockDropZone::Left => UiRect {
            width: rect.width * 0.5,
            ..rect
        },
        DockDropZone::Right => UiRect {
            x: rect.x + rect.width * 0.5,
            width: rect.width * 0.5,
            ..rect
        },
        DockDropZone::Top => UiRect {
            height: rect.height * 0.5,
            ..rect
        },
        DockDropZone::Bottom => UiRect {
            y: rect.y + rect.height * 0.5,
            height: rect.height * 0.5,
            ..rect
        },
    }
}

fn is_stack(node: &DockNode, node_id: &LayoutNodeId) -> bool {
    match node {
        DockNode::Split { first, second, .. } => {
            is_stack(first, node_id) || is_stack(second, node_id)
        }
        DockNode::Stack {
            node_id: candidate, ..
        } => candidate == node_id,
    }
}

fn find_stack_mut<'a>(node: &'a mut DockNode, node_id: &LayoutNodeId) -> Option<&'a mut DockNode> {
    match node {
        DockNode::Split { first, second, .. } => {
            if let Some(stack) = find_stack_mut(first, node_id) {
                Some(stack)
            } else {
                find_stack_mut(second, node_id)
            }
        }
        DockNode::Stack {
            node_id: candidate, ..
        } => (candidate == node_id).then_some(node),
    }
}

fn first_stack_mut(node: &mut DockNode) -> Option<&mut DockNode> {
    match node {
        DockNode::Split { first, second, .. } => {
            if let Some(stack) = first_stack_mut(first) {
                Some(stack)
            } else {
                first_stack_mut(second)
            }
        }
        DockNode::Stack { .. } => Some(node),
    }
}

fn remove_panel(node: DockNode, panel_id: &PanelId) -> Option<DockNode> {
    match node {
        DockNode::Stack {
            node_id,
            mut tabs,
            mut active_panel_id,
        } => {
            let removed_active = active_panel_id == *panel_id;
            tabs.retain(|tab| tab != panel_id);
            if tabs.is_empty() {
                None
            } else {
                if removed_active {
                    active_panel_id = tabs[0].clone();
                }
                Some(DockNode::Stack {
                    node_id,
                    tabs,
                    active_panel_id,
                })
            }
        }
        DockNode::Split {
            node_id,
            axis,
            ratio,
            first,
            second,
        } => match (
            remove_panel(*first, panel_id),
            remove_panel(*second, panel_id),
        ) {
            (Some(first), Some(second)) => Some(DockNode::Split {
                node_id,
                axis,
                ratio,
                first: Box::new(first),
                second: Box::new(second),
            }),
            (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
            (None, None) => None,
        },
    }
}

pub fn validate_workspace_topology(
    topology: &WorkspaceTopology,
    registry: &PanelRegistry,
) -> Result<(), Vec<WorkspaceLayoutDiagnostic>> {
    let mut diagnostics = Vec::new();
    if topology.schema_version != EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION {
        diagnostics.push(WorkspaceLayoutDiagnostic::new(
            "invalid_topology_schema",
            None,
            None,
        ));
    }
    if let Err(mut main_diagnostics) = validate_workspace_layout(&topology.main_root, registry) {
        diagnostics.append(&mut main_diagnostics);
    }
    let mut node_ids = BTreeSet::new();
    let mut visible_panels = BTreeSet::new();
    collect_node_and_panel_ids(&topology.main_root.root, &mut node_ids, &mut visible_panels);
    let mut window_ids = BTreeSet::new();
    for floating in &topology.floating_roots {
        if floating.window_id.is_main() || !window_ids.insert(floating.window_id.clone()) {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "duplicate_workspace_window",
                Some(floating.root.node_id()),
                None,
            ));
        }
        validate_node(
            &floating.root,
            registry,
            &mut node_ids,
            &mut visible_panels,
            &mut diagnostics,
        );
    }
    for closed in &topology.main_root.closed_panels {
        if visible_panels.contains(closed) {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "panel_visible_and_closed",
                None,
                Some(closed),
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_floating_root(
    floating: &WorkspaceWindowRoot,
    registry: &PanelRegistry,
) -> Result<(), Vec<WorkspaceLayoutDiagnostic>> {
    let layout = EditorWorkspaceLayout {
        schema_version: EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION.to_string(),
        root: floating.root.clone(),
        closed_panels: Vec::new(),
    };
    validate_workspace_layout(&layout, registry)
}

fn collect_node_and_panel_ids(
    node: &DockNode,
    node_ids: &mut BTreeSet<LayoutNodeId>,
    panels: &mut BTreeSet<PanelId>,
) {
    node_ids.insert(node.node_id().clone());
    match node {
        DockNode::Split { first, second, .. } => {
            collect_node_and_panel_ids(first, node_ids, panels);
            collect_node_and_panel_ids(second, node_ids, panels);
        }
        DockNode::Stack { tabs, .. } => {
            panels.extend(tabs.iter().cloned());
        }
    }
}

fn topology_panel_location(
    main: &EditorWorkspaceLayout,
    floating: &BTreeMap<WorkspaceWindowId, WorkspaceWindowRoot>,
    panel_id: &PanelId,
) -> Option<WorkspaceWindowId> {
    if stack_id_containing(&main.root, panel_id).is_some() {
        Some(WorkspaceWindowId::main())
    } else {
        floating_window_containing(floating, panel_id).cloned()
    }
}

fn floating_window_containing<'a>(
    floating: &'a BTreeMap<WorkspaceWindowId, WorkspaceWindowRoot>,
    panel_id: &PanelId,
) -> Option<&'a WorkspaceWindowId> {
    floating
        .iter()
        .find(|(_, root)| stack_id_containing(&root.root, panel_id).is_some())
        .map(|(window_id, _)| window_id)
}

fn collect_panels(node: &DockNode, panels: &mut Vec<PanelId>) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_panels(first, panels);
            collect_panels(second, panels);
        }
        DockNode::Stack { tabs, .. } => panels.extend(tabs.iter().cloned()),
    }
}

fn collect_panels_into_set(node: &DockNode, panels: &mut BTreeSet<PanelId>) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_panels_into_set(first, panels);
            collect_panels_into_set(second, panels);
        }
        DockNode::Stack { tabs, .. } => panels.extend(tabs.iter().cloned()),
    }
}

fn insert_panel_into_stack(
    node: &mut DockNode,
    stack_id: &LayoutNodeId,
    panel_id: &PanelId,
) -> bool {
    let Some(DockNode::Stack {
        tabs,
        active_panel_id,
        ..
    }) = find_stack_mut(node, stack_id)
    else {
        return false;
    };
    tabs.push(panel_id.clone());
    *active_panel_id = panel_id.clone();
    true
}

fn insert_panel_at_default(
    main_root: &mut DockNode,
    panel_id: &PanelId,
    registry: &PanelRegistry,
) -> bool {
    let Some(descriptor) = registry.get(panel_id.as_str()) else {
        return false;
    };
    let preferred = layout_node_id(&format!("workspace/{}", descriptor.default_stack_id));
    if is_stack(main_root, &preferred) {
        insert_panel_into_stack(main_root, &preferred, panel_id)
    } else {
        let existing = main_root.clone();
        let new_stack = DockNode::Stack {
            node_id: preferred,
            tabs: vec![panel_id.clone()],
            active_panel_id: panel_id.clone(),
        };
        let (axis, ratio, first, second) = match descriptor.default_stack_id.as_str() {
            "left" => (DockSplitAxis::Horizontal, 0.20, new_stack, existing),
            "right" => (DockSplitAxis::Horizontal, 0.80, existing, new_stack),
            "bottom" => (DockSplitAxis::Vertical, 0.72, existing, new_stack),
            _ => (DockSplitAxis::Horizontal, 0.67, existing, new_stack),
        };
        *main_root = DockNode::Split {
            node_id: layout_node_id(&format!("workspace/rehome/{}/split", panel_id.as_str())),
            axis,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        };
        true
    }
}

fn sanitize_placement(mut placement: WorkspaceWindowPlacement) -> WorkspaceWindowPlacement {
    if !placement.x.is_finite() {
        placement.x = 120.0;
    }
    if !placement.y.is_finite() {
        placement.y = 80.0;
    }
    placement.width = if placement.width.is_finite() {
        placement.width.max(240.0)
    } else {
        640.0
    };
    placement.height = if placement.height.is_finite() {
        placement.height.max(180.0)
    } else {
        480.0
    };
    placement
}

fn placement_from_rect(rect: UiRect, display_id: Option<String>) -> WorkspaceWindowPlacement {
    WorkspaceWindowPlacement {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        display_id,
    }
}

fn clamp_placement(
    placement: &WorkspaceWindowPlacement,
    displays: &[WorkspaceDisplay],
) -> WorkspaceWindowPlacement {
    let mut placement = sanitize_placement(placement.clone());
    let display = placement
        .display_id
        .as_deref()
        .and_then(|id| displays.iter().find(|display| display.display_id == id))
        .or_else(|| displays.first());
    let Some(display) = display else {
        return placement;
    };
    placement.display_id = Some(display.display_id.clone());
    placement.width = placement.width.min(display.work_area.width.max(240.0));
    placement.height = placement.height.min(display.work_area.height.max(180.0));
    placement.x = placement.x.clamp(
        display.work_area.x,
        (display.work_area.x + display.work_area.width - placement.width).max(display.work_area.x),
    );
    placement.y = placement.y.clamp(
        display.work_area.y,
        (display.work_area.y + display.work_area.height - placement.height)
            .max(display.work_area.y),
    );
    placement
}

fn reconcile_workspace_layout(
    mut layout: EditorWorkspaceLayout,
    registry: &PanelRegistry,
) -> Result<(EditorWorkspaceLayout, Vec<WorkspaceLayoutDiagnostic>), Vec<WorkspaceLayoutDiagnostic>>
{
    if layout.schema_version != EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION {
        return Err(vec![WorkspaceLayoutDiagnostic::new(
            "invalid_layout_schema",
            None,
            None,
        )]);
    }
    let mut diagnostics = Vec::new();
    let Some(root) = reconcile_workspace_node(layout.root, registry, &mut diagnostics) else {
        diagnostics.push(WorkspaceLayoutDiagnostic::new(
            "empty_reconciled_layout",
            None,
            None,
        ));
        return Err(diagnostics);
    };
    layout.root = root;
    layout.closed_panels.retain(|panel_id| {
        if registry.contains(panel_id.as_str()) {
            true
        } else {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "removed_unknown_panel",
                None,
                Some(panel_id),
            ));
            false
        }
    });

    let mut present = BTreeSet::new();
    collect_panel_ids(&layout.root, &mut present);
    present.extend(layout.closed_panels.iter().cloned());
    for panel_id in registry.panel_ids() {
        if present.contains(panel_id) {
            continue;
        }
        let descriptor = registry
            .get(panel_id.as_str())
            .expect("registry iterator yields registered panel");
        let preferred_stack_id =
            layout_node_id(&format!("workspace/{}", descriptor.default_stack_id));
        let target = if is_stack(&layout.root, &preferred_stack_id) {
            find_stack_mut(&mut layout.root, &preferred_stack_id)
        } else {
            first_stack_mut(&mut layout.root)
        };
        let Some(DockNode::Stack { tabs, .. }) = target else {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "panel_default_stack_missing",
                Some(&preferred_stack_id),
                Some(panel_id),
            ));
            return Err(diagnostics);
        };
        tabs.push(panel_id.clone());
        diagnostics.push(WorkspaceLayoutDiagnostic::new(
            "added_new_panel",
            Some(&preferred_stack_id),
            Some(panel_id),
        ));
        present.insert(panel_id.clone());
    }
    if let Err(mut invalid) = validate_workspace_layout(&layout, registry) {
        diagnostics.append(&mut invalid);
        return Err(diagnostics);
    }
    Ok((layout, diagnostics))
}

fn reconcile_workspace_node(
    node: DockNode,
    registry: &PanelRegistry,
    diagnostics: &mut Vec<WorkspaceLayoutDiagnostic>,
) -> Option<DockNode> {
    match node {
        DockNode::Stack {
            node_id,
            mut tabs,
            mut active_panel_id,
        } => {
            tabs.retain(|panel_id| {
                if registry.contains(panel_id.as_str()) {
                    true
                } else {
                    diagnostics.push(WorkspaceLayoutDiagnostic::new(
                        "removed_unknown_panel",
                        Some(&node_id),
                        Some(panel_id),
                    ));
                    false
                }
            });
            if tabs.is_empty() {
                return None;
            }
            if !tabs.contains(&active_panel_id) {
                active_panel_id = tabs[0].clone();
                diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "active_panel_repaired",
                    Some(&node_id),
                    Some(&active_panel_id),
                ));
            }
            Some(DockNode::Stack {
                node_id,
                tabs,
                active_panel_id,
            })
        }
        DockNode::Split {
            node_id,
            axis,
            mut ratio,
            first,
            second,
        } => {
            let original_ratio = ratio;
            ratio = if ratio.is_finite() {
                ratio.clamp(0.05, 0.95)
            } else {
                0.5
            };
            if ratio != original_ratio {
                diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "split_ratio_clamped",
                    Some(&node_id),
                    None,
                ));
            }
            match (
                reconcile_workspace_node(*first, registry, diagnostics),
                reconcile_workspace_node(*second, registry, diagnostics),
            ) {
                (Some(first), Some(second)) => Some(DockNode::Split {
                    node_id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            }
        }
    }
}

fn collect_panel_ids(node: &DockNode, panel_ids: &mut BTreeSet<PanelId>) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_panel_ids(first, panel_ids);
            collect_panel_ids(second, panel_ids);
        }
        DockNode::Stack { tabs, .. } => panel_ids.extend(tabs.iter().cloned()),
    }
}

fn replace_stack_with_split(
    node: &mut DockNode,
    target_node_id: &LayoutNodeId,
    panel_id: &PanelId,
    zone: DockDropZone,
    next_revision: u64,
) -> bool {
    if node.node_id() == target_node_id && matches!(node, DockNode::Stack { .. }) {
        let existing = node.clone();
        let new_stack = DockNode::Stack {
            node_id: layout_node_id(&format!(
                "workspace/stack/{}/{}",
                panel_id.as_str(),
                next_revision
            )),
            tabs: vec![panel_id.clone()],
            active_panel_id: panel_id.clone(),
        };
        let (axis, ratio, first, second) = match zone {
            DockDropZone::Left => (DockSplitAxis::Horizontal, 0.25, new_stack, existing),
            DockDropZone::Right => (DockSplitAxis::Horizontal, 0.75, existing, new_stack),
            DockDropZone::Top => (DockSplitAxis::Vertical, 0.25, new_stack, existing),
            DockDropZone::Bottom => (DockSplitAxis::Vertical, 0.75, existing, new_stack),
            DockDropZone::Center => return false,
        };
        *node = DockNode::Split {
            node_id: layout_node_id(&format!(
                "workspace/split/{}/{}",
                panel_id.as_str(),
                next_revision
            )),
            axis,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        };
        return true;
    }
    match node {
        DockNode::Split { first, second, .. } => {
            replace_stack_with_split(first, target_node_id, panel_id, zone, next_revision)
                || replace_stack_with_split(second, target_node_id, panel_id, zone, next_revision)
        }
        DockNode::Stack { .. } => false,
    }
}

fn split_resize_inputs(
    node: &DockNode,
    node_id: &LayoutNodeId,
    registry: &PanelRegistry,
) -> Option<(DockSplitAxis, f32, PanelSize, PanelSize)> {
    match node {
        DockNode::Split {
            node_id: candidate,
            axis,
            ratio,
            first,
            second,
        } if candidate == node_id => Some((
            *axis,
            *ratio,
            minimum_size(first, registry),
            minimum_size(second, registry),
        )),
        DockNode::Split { first, second, .. } => split_resize_inputs(first, node_id, registry)
            .or_else(|| split_resize_inputs(second, node_id, registry)),
        DockNode::Stack { .. } => None,
    }
}

fn set_split_ratio(node: &mut DockNode, node_id: &LayoutNodeId, ratio: f32) -> bool {
    match node {
        DockNode::Split {
            node_id: candidate,
            ratio: current,
            ..
        } if candidate == node_id => {
            if (*current - ratio).abs() <= f32::EPSILON {
                false
            } else {
                *current = ratio;
                true
            }
        }
        DockNode::Split { first, second, .. } => {
            set_split_ratio(first, node_id, ratio) || set_split_ratio(second, node_id, ratio)
        }
        DockNode::Stack { .. } => false,
    }
}

fn pointer_axis(pointer: UiPoint, axis: DockSplitAxis) -> f32 {
    match axis {
        DockSplitAxis::Horizontal => pointer.x,
        DockSplitAxis::Vertical => pointer.y,
    }
}

fn axis_extent(rect: UiRect, axis: DockSplitAxis) -> f32 {
    match axis {
        DockSplitAxis::Horizontal => rect.width,
        DockSplitAxis::Vertical => rect.height,
    }
}

fn axis_size(size: PanelSize, axis: DockSplitAxis) -> f32 {
    match axis {
        DockSplitAxis::Horizontal => size.width,
        DockSplitAxis::Vertical => size.height,
    }
}

pub fn validate_workspace_layout(
    layout: &EditorWorkspaceLayout,
    registry: &PanelRegistry,
) -> Result<(), Vec<WorkspaceLayoutDiagnostic>> {
    let mut diagnostics = Vec::new();
    if layout.schema_version != EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION {
        diagnostics.push(WorkspaceLayoutDiagnostic::new(
            "invalid_layout_schema",
            None,
            None,
        ));
    }
    let mut node_ids = BTreeSet::new();
    let mut visible_panels = BTreeSet::new();
    validate_node(
        &layout.root,
        registry,
        &mut node_ids,
        &mut visible_panels,
        &mut diagnostics,
    );
    let mut closed_panels = BTreeSet::new();
    for panel_id in &layout.closed_panels {
        if !registry.contains(panel_id.as_str()) {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "unknown_panel",
                None,
                Some(panel_id),
            ));
        }
        if !closed_panels.insert(panel_id.clone()) {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "duplicate_closed_panel",
                None,
                Some(panel_id),
            ));
        }
        if visible_panels.contains(panel_id) {
            diagnostics.push(WorkspaceLayoutDiagnostic::new(
                "panel_visible_and_closed",
                None,
                Some(panel_id),
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_node(
    node: &DockNode,
    registry: &PanelRegistry,
    node_ids: &mut BTreeSet<LayoutNodeId>,
    visible_panels: &mut BTreeSet<PanelId>,
    diagnostics: &mut Vec<WorkspaceLayoutDiagnostic>,
) {
    let node_id = node.node_id();
    if node_id.as_str().trim().is_empty() || !node_ids.insert(node_id.clone()) {
        diagnostics.push(WorkspaceLayoutDiagnostic::new(
            "duplicate_layout_node",
            Some(node_id),
            None,
        ));
    }
    match node {
        DockNode::Split {
            ratio,
            first,
            second,
            ..
        } => {
            if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "invalid_split_ratio",
                    Some(node_id),
                    None,
                ));
            }
            validate_node(first, registry, node_ids, visible_panels, diagnostics);
            validate_node(second, registry, node_ids, visible_panels, diagnostics);
        }
        DockNode::Stack {
            tabs,
            active_panel_id,
            ..
        } => {
            if tabs.is_empty() {
                diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "empty_stack",
                    Some(node_id),
                    None,
                ));
            }
            if !tabs.iter().any(|panel_id| panel_id == active_panel_id) {
                diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "invalid_active_tab",
                    Some(node_id),
                    Some(active_panel_id),
                ));
            }
            for panel_id in tabs {
                if !registry.contains(panel_id.as_str()) {
                    diagnostics.push(WorkspaceLayoutDiagnostic::new(
                        "unknown_panel",
                        Some(node_id),
                        Some(panel_id),
                    ));
                }
                if !visible_panels.insert(panel_id.clone()) {
                    diagnostics.push(WorkspaceLayoutDiagnostic::new(
                        "duplicate_panel",
                        Some(node_id),
                        Some(panel_id),
                    ));
                }
            }
        }
    }
}

fn resolve_node(
    node: &DockNode,
    rect: UiRect,
    registry: &PanelRegistry,
    snapshot: &mut WorkspaceSnapshot,
) -> PanelSize {
    snapshot.node_rects.insert(node.node_id().clone(), rect);
    match node {
        DockNode::Stack {
            node_id,
            tabs,
            active_panel_id,
        } => {
            snapshot
                .active_tabs
                .insert(node_id.clone(), active_panel_id.clone());
            for panel_id in tabs {
                snapshot.panel_rects.insert(panel_id.clone(), rect);
            }
            tabs.iter()
                .filter_map(|panel_id| registry.get(panel_id.as_str()))
                .fold(
                    PanelSize {
                        width: 0.0,
                        height: 0.0,
                    },
                    |minimum, descriptor| PanelSize {
                        width: minimum.width.max(descriptor.minimum_size.width),
                        height: minimum.height.max(descriptor.minimum_size.height),
                    },
                )
        }
        DockNode::Split {
            node_id,
            axis,
            ratio,
            first,
            second,
        } => {
            let first_minimum = minimum_size(first, registry);
            let second_minimum = minimum_size(second, registry);
            let available = match axis {
                DockSplitAxis::Horizontal => rect.width,
                DockSplitAxis::Vertical => rect.height,
            }
            .max(0.0);
            let desired = available * ratio.clamp(0.0, 1.0);
            let first_minimum_axis = match axis {
                DockSplitAxis::Horizontal => first_minimum.width,
                DockSplitAxis::Vertical => first_minimum.height,
            };
            let second_minimum_axis = match axis {
                DockSplitAxis::Horizontal => second_minimum.width,
                DockSplitAxis::Vertical => second_minimum.height,
            };
            let maximum = (available - second_minimum_axis).max(0.0);
            let split = if first_minimum_axis <= maximum {
                desired.clamp(first_minimum_axis, maximum)
            } else {
                snapshot.diagnostics.push(WorkspaceLayoutDiagnostic::new(
                    "minimum_size_unsatisfied",
                    Some(node_id),
                    None,
                ));
                desired.clamp(0.0, available)
            };
            let (first_rect, second_rect, hit_rect, visual_rect) = split_rect(rect, *axis, split);
            snapshot.splitters.push(WorkspaceSplitter {
                node_id: node_id.clone(),
                axis: *axis,
                hit_rect,
                visual_rect,
            });
            resolve_node(first, first_rect, registry, snapshot);
            resolve_node(second, second_rect, registry, snapshot);
            match axis {
                DockSplitAxis::Horizontal => PanelSize {
                    width: first_minimum.width + second_minimum.width,
                    height: first_minimum.height.max(second_minimum.height),
                },
                DockSplitAxis::Vertical => PanelSize {
                    width: first_minimum.width.max(second_minimum.width),
                    height: first_minimum.height + second_minimum.height,
                },
            }
        }
    }
}

fn minimum_size(node: &DockNode, registry: &PanelRegistry) -> PanelSize {
    match node {
        DockNode::Stack { tabs, .. } => tabs
            .iter()
            .filter_map(|panel_id| registry.get(panel_id.as_str()))
            .fold(
                PanelSize {
                    width: 0.0,
                    height: 0.0,
                },
                |minimum, descriptor| PanelSize {
                    width: minimum.width.max(descriptor.minimum_size.width),
                    height: minimum.height.max(descriptor.minimum_size.height),
                },
            ),
        DockNode::Split {
            axis,
            first,
            second,
            ..
        } => {
            let first = minimum_size(first, registry);
            let second = minimum_size(second, registry);
            match axis {
                DockSplitAxis::Horizontal => PanelSize {
                    width: first.width + second.width,
                    height: first.height.max(second.height),
                },
                DockSplitAxis::Vertical => PanelSize {
                    width: first.width.max(second.width),
                    height: first.height + second.height,
                },
            }
        }
    }
}

fn split_rect(rect: UiRect, axis: DockSplitAxis, split: f32) -> (UiRect, UiRect, UiRect, UiRect) {
    const HIT_WIDTH: f32 = 7.0;
    const VISUAL_WIDTH: f32 = 1.0;
    match axis {
        DockSplitAxis::Horizontal => {
            let center = rect.x + split;
            let hit_width = HIT_WIDTH.min(rect.width.max(0.0));
            let visual_width = VISUAL_WIDTH.min(hit_width);
            (
                UiRect {
                    width: split,
                    ..rect
                },
                UiRect {
                    x: rect.x + split,
                    width: (rect.width - split).max(0.0),
                    ..rect
                },
                UiRect {
                    x: center - hit_width * 0.5,
                    y: rect.y,
                    width: hit_width,
                    height: rect.height,
                },
                UiRect {
                    x: center - visual_width * 0.5,
                    y: rect.y,
                    width: visual_width,
                    height: rect.height,
                },
            )
        }
        DockSplitAxis::Vertical => {
            let center = rect.y + split;
            let hit_height = HIT_WIDTH.min(rect.height.max(0.0));
            let visual_height = VISUAL_WIDTH.min(hit_height);
            (
                UiRect {
                    height: split,
                    ..rect
                },
                UiRect {
                    y: rect.y + split,
                    height: (rect.height - split).max(0.0),
                    ..rect
                },
                UiRect {
                    x: rect.x,
                    y: center - hit_height * 0.5,
                    width: rect.width,
                    height: hit_height,
                },
                UiRect {
                    x: rect.x,
                    y: center - visual_height * 0.5,
                    width: rect.width,
                    height: visual_height,
                },
            )
        }
    }
}

fn default_workspace_layout(registry: &PanelRegistry) -> EditorWorkspaceLayout {
    let mut bottom_tabs = Vec::new();
    for panel_id in registry.panel_ids() {
        if !matches!(panel_id.as_str(), "hierarchy" | "viewport" | "inspector") {
            bottom_tabs.push(panel_id.clone());
        }
    }
    let bottom_active = if bottom_tabs
        .iter()
        .any(|panel_id| panel_id.as_str() == "asset_browser")
    {
        panel_id("asset_browser")
    } else {
        bottom_tabs
            .first()
            .cloned()
            .expect("standard editor has bottom panels")
    };
    EditorWorkspaceLayout {
        schema_version: EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION.to_string(),
        root: DockNode::Split {
            node_id: layout_node_id("workspace/root"),
            axis: DockSplitAxis::Vertical,
            ratio: 0.72,
            first: Box::new(DockNode::Split {
                node_id: layout_node_id("workspace/top"),
                axis: DockSplitAxis::Horizontal,
                ratio: 0.20,
                first: Box::new(stack("workspace/left", &["hierarchy"], "hierarchy")),
                second: Box::new(DockNode::Split {
                    node_id: layout_node_id("workspace/top-main"),
                    axis: DockSplitAxis::Horizontal,
                    ratio: 0.67,
                    first: Box::new(stack("workspace/center", &["viewport"], "viewport")),
                    second: Box::new(stack("workspace/right", &["inspector"], "inspector")),
                }),
            }),
            second: Box::new(DockNode::Stack {
                node_id: layout_node_id("workspace/bottom"),
                tabs: bottom_tabs,
                active_panel_id: bottom_active,
            }),
        },
        closed_panels: Vec::new(),
    }
}

fn stack(node_id: &str, tabs: &[&str], active_panel_id: &str) -> DockNode {
    DockNode::Stack {
        node_id: layout_node_id(node_id),
        tabs: tabs.iter().map(|panel| panel_id(panel)).collect(),
        active_panel_id: panel_id(active_panel_id),
    }
}

fn layout_node_id(value: &str) -> LayoutNodeId {
    LayoutNodeId::new(value).expect("built-in layout node id")
}

fn panel_id(value: &str) -> PanelId {
    PanelId::new(value).expect("built-in panel id")
}

fn default_descriptor(panel_id: &str) -> PanelDescriptor {
    let (title, minimum_size, preferred_size, default_stack_id, closable) = match panel_id {
        "hierarchy" => ("Hierarchy", (180.0, 160.0), (260.0, 520.0), "left", false),
        "viewport" => ("Viewport", (240.0, 180.0), (760.0, 520.0), "center", false),
        "inspector" => ("Inspector", (220.0, 160.0), (340.0, 520.0), "right", false),
        "ai_panel" => ("AI", (220.0, 140.0), (340.0, 320.0), "right", true),
        "asset_browser" => ("Project", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "authoring_workflow" => ("Workflow", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "input_mapping" => ("Input", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "build_export" => ("Build", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "runtime_trace" => ("Trace", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "project_intent" => ("Intent", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        "report" => ("Report", (240.0, 120.0), (720.0, 220.0), "bottom", true),
        _ => (panel_id, (240.0, 120.0), (720.0, 220.0), "bottom", true),
    };
    PanelDescriptor {
        panel_id: self::panel_id(panel_id),
        title: title.to_string(),
        minimum_size: PanelSize {
            width: minimum_size.0,
            height: minimum_size.1,
        },
        preferred_size: PanelSize {
            width: preferred_size.0,
            height: preferred_size.1,
        },
        closable,
        default_stack_id: default_stack_id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(panel_id: &str) -> PanelDescriptor {
        PanelDescriptor {
            panel_id: self::panel_id(panel_id),
            title: panel_id.to_string(),
            minimum_size: PanelSize {
                width: 120.0,
                height: 80.0,
            },
            preferred_size: PanelSize {
                width: 320.0,
                height: 240.0,
            },
            closable: true,
            default_stack_id: "main".to_string(),
        }
    }

    #[test]
    fn workspace_docking_rejects_duplicate_panel_across_stacks() {
        let mut registry = PanelRegistry::default();
        assert!(registry.register(descriptor("viewport")));
        let layout = EditorWorkspaceLayout {
            schema_version: EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION.to_string(),
            root: DockNode::Split {
                node_id: layout_node_id("root"),
                axis: DockSplitAxis::Horizontal,
                ratio: 0.5,
                first: Box::new(stack("left", &["viewport"], "viewport")),
                second: Box::new(stack("right", &["viewport"], "viewport")),
            },
            closed_panels: Vec::new(),
        };

        let diagnostics =
            validate_workspace_layout(&layout, &registry).expect_err("duplicate panel must fail");
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate_panel"));
    }

    #[test]
    fn workspace_docking_rejects_empty_stack_invalid_active_and_ratio() {
        let registry = PanelRegistry::default();
        let layout = EditorWorkspaceLayout {
            schema_version: EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION.to_string(),
            root: DockNode::Split {
                node_id: layout_node_id("root"),
                axis: DockSplitAxis::Vertical,
                ratio: f32::NAN,
                first: Box::new(stack("empty", &[], "missing")),
                second: Box::new(stack("invalid-active", &["unknown"], "missing")),
            },
            closed_panels: Vec::new(),
        };

        let diagnostics =
            validate_workspace_layout(&layout, &registry).expect_err("invalid layout must fail");
        for code in [
            "invalid_split_ratio",
            "empty_stack",
            "invalid_active_tab",
            "unknown_panel",
        ] {
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.code == code),
                "missing diagnostic {code}"
            );
        }
    }

    #[test]
    fn workspace_docking_default_contains_each_manifest_panel_once() {
        let module = EditorWorkspaceDockingModule::standard_editor();
        let dockable_manifest_count = native_editor_panel_manifest()
            .iter()
            .filter(|entry| entry.dockable)
            .count();
        assert_eq!(module.registry().len(), dockable_manifest_count);
        validate_workspace_layout(module.layout(), module.registry()).expect("default layout");
        let snapshot = module.snapshot(UiRect {
            x: 0.0,
            y: 52.0,
            width: 1600.0,
            height: 848.0,
        });
        assert_eq!(snapshot.panel_rects.len(), module.registry().len());
        assert!(snapshot.panel_rects.contains_key("viewport"));
        assert!(snapshot.panel_rects.contains_key("project_intent"));
        assert!(snapshot.drag_preview.is_none());
        assert!(snapshot.diagnostics.is_empty());
    }

    #[test]
    fn workspace_docking_sanitizes_geometry_and_reports_minimum_pressure() {
        assert!(LayoutNodeId::new("").is_none());
        assert!(PanelId::new("  ").is_none());
        let module = EditorWorkspaceDockingModule::standard_editor();
        let snapshot = module.snapshot(UiRect {
            x: f32::NAN,
            y: f32::INFINITY,
            width: 100.0,
            height: 80.0,
        });
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "minimum_size_unsatisfied"));
        for rect in snapshot
            .node_rects
            .values()
            .chain(snapshot.panel_rects.values())
            .chain(
                snapshot
                    .splitters
                    .iter()
                    .flat_map(|splitter| [&splitter.hit_rect, &splitter.visual_rect]),
            )
        {
            assert!(rect.x.is_finite());
            assert!(rect.y.is_finite());
            assert!(rect.width.is_finite() && rect.width >= 0.0);
            assert!(rect.height.is_finite() && rect.height >= 0.0);
        }
    }

    #[test]
    fn workspace_docking_splitter_separates_visual_and_hit_geometry() {
        let module = EditorWorkspaceDockingModule::standard_editor();
        let snapshot = module.snapshot(UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        });

        for splitter in &snapshot.splitters {
            let (hit_thickness, visual_thickness, hit_center, visual_center) = match splitter.axis {
                DockSplitAxis::Horizontal => (
                    splitter.hit_rect.width,
                    splitter.visual_rect.width,
                    splitter.hit_rect.x + splitter.hit_rect.width * 0.5,
                    splitter.visual_rect.x + splitter.visual_rect.width * 0.5,
                ),
                DockSplitAxis::Vertical => (
                    splitter.hit_rect.height,
                    splitter.visual_rect.height,
                    splitter.hit_rect.y + splitter.hit_rect.height * 0.5,
                    splitter.visual_rect.y + splitter.visual_rect.height * 0.5,
                ),
            };

            assert_eq!(hit_thickness, 7.0);
            assert_eq!(visual_thickness, 1.0);
            assert!((hit_center - visual_center).abs() <= f32::EPSILON);
            assert_eq!(
                splitter.hit_rect.intersection(splitter.visual_rect),
                Some(splitter.visual_rect)
            );
        }
    }

    #[test]
    fn workspace_docking_resizes_both_axes_clamps_and_cancels() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };

        for (node, delta) in [
            ("workspace/root", (0.0, -80.0)),
            ("workspace/top", (-500.0, 0.0)),
        ] {
            let node_id = layout_node_id(node);
            let before = split_ratio(module.layout(), &node_id);
            let splitter = module
                .snapshot(workspace)
                .splitters
                .into_iter()
                .find(|splitter| splitter.node_id == node_id)
                .expect("default splitter");
            let pointer = UiPoint {
                x: splitter.hit_rect.x + splitter.hit_rect.width * 0.5,
                y: splitter.hit_rect.y + splitter.hit_rect.height * 0.5,
            };
            module.update(WorkspaceIntent::BeginSplitterResize {
                node_id: node_id.clone(),
                pointer,
                workspace_rect: workspace,
            });
            assert_eq!(module.active_resize_node_id(), Some(&node_id));
            let update = module.update(WorkspaceIntent::UpdateSplitterResize {
                pointer: UiPoint {
                    x: pointer.x + delta.0,
                    y: pointer.y + delta.1,
                },
            });
            assert!(update.changed);
            assert_ne!(split_ratio(module.layout(), &node_id), before);
            module.update(WorkspaceIntent::CancelSplitterResize);
            assert_eq!(split_ratio(module.layout(), &node_id), before);
            assert!(module.active_resize_node_id().is_none());
        }

        let snapshot = module.snapshot(workspace);
        assert!(
            snapshot.node_rects["workspace/left"].width
                >= module
                    .registry()
                    .get("hierarchy")
                    .unwrap()
                    .minimum_size
                    .width
        );
    }

    #[test]
    fn workspace_docking_pointer_up_commits_ratio_and_window_resize_preserves_it() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };
        let node_id = layout_node_id("workspace/top-main");
        let splitter = module
            .snapshot(workspace)
            .splitters
            .into_iter()
            .find(|splitter| splitter.node_id == node_id)
            .unwrap();
        let pointer = UiPoint {
            x: splitter.hit_rect.x + splitter.hit_rect.width * 0.5,
            y: splitter.hit_rect.y + splitter.hit_rect.height * 0.5,
        };
        module.update(WorkspaceIntent::BeginSplitterResize {
            node_id: node_id.clone(),
            pointer,
            workspace_rect: workspace,
        });
        module.update(WorkspaceIntent::UpdateSplitterResize {
            pointer: UiPoint {
                x: pointer.x - 40.0,
                y: pointer.y,
            },
        });
        let committed_ratio = split_ratio(module.layout(), &node_id);
        module.update(WorkspaceIntent::CommitSplitterResize);
        assert!(module.active_resize_node_id().is_none());
        let _ = module.snapshot(UiRect {
            width: 1600.0,
            height: 848.0,
            ..workspace
        });
        assert_eq!(split_ratio(module.layout(), &node_id), committed_ratio);
    }

    fn split_ratio(layout: &EditorWorkspaceLayout, node_id: &LayoutNodeId) -> f32 {
        fn find(node: &DockNode, node_id: &LayoutNodeId) -> Option<f32> {
            match node {
                DockNode::Split {
                    node_id: candidate,
                    ratio,
                    first,
                    second,
                    ..
                } => {
                    if candidate == node_id {
                        Some(*ratio)
                    } else {
                        find(first, node_id).or_else(|| find(second, node_id))
                    }
                }
                DockNode::Stack { .. } => None,
            }
        }
        find(&layout.root, node_id).expect("split ratio")
    }

    #[test]
    fn workspace_layout_schema_restore_falls_back_on_invalid_schema() {
        let registry = PanelRegistry::standard_editor();
        let invalid = EditorWorkspaceLayout {
            schema_version: "future".to_string(),
            root: stack("invalid", &["viewport"], "viewport"),
            closed_panels: Vec::new(),
        };
        let (module, restore) =
            EditorWorkspaceDockingModule::restore_or_default(registry, Some(invalid));
        assert!(restore.used_default);
        assert!(restore
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_layout_schema"));
        validate_workspace_layout(module.layout(), module.registry()).expect("fallback layout");
    }

    #[test]
    fn workspace_docking_panel_drag_preview_and_commit_share_target() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };
        let snapshot = module.snapshot(workspace);
        let source_rect = snapshot.node_rects["workspace/bottom"];
        let target_rect = snapshot.node_rects["workspace/left"];
        let start = UiPoint {
            x: source_rect.x + 20.0,
            y: source_rect.y + 12.0,
        };
        let target = UiPoint {
            x: target_rect.x + target_rect.width * 0.5,
            y: target_rect.y + target_rect.height * 0.5,
        };

        module.update(WorkspaceIntent::BeginPanelDrag {
            panel_id: panel_id("ai_panel"),
            pointer: start,
            workspace_rect: workspace,
        });
        let update = module.update(WorkspaceIntent::UpdatePanelDrag {
            pointer: target,
            workspace_rect: workspace,
        });
        assert!(update.diagnostics.is_empty());
        let preview = module
            .snapshot(workspace)
            .drag_preview
            .expect("drag preview");
        assert_eq!(preview.target_node_id.as_str(), "workspace/left");
        assert_eq!(preview.zone, DockDropZone::Center);

        let update = module.update(WorkspaceIntent::CommitPanelDrag);
        assert!(update.changed);
        assert!(update.diagnostics.is_empty());
        assert_eq!(
            module
                .active_panel_id("workspace/left")
                .map(PanelId::as_str),
            Some("ai_panel")
        );
        assert!(module.snapshot(workspace).drag_preview.is_none());
    }

    #[test]
    fn workspace_docking_panel_drag_threshold_cancel_and_same_stack_reorder() {
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let bottom = module.snapshot(workspace).node_rects["workspace/bottom"];
        let start = UiPoint {
            x: bottom.x + 20.0,
            y: bottom.y + 12.0,
        };

        module.update(WorkspaceIntent::BeginPanelDrag {
            panel_id: panel_id("console"),
            pointer: start,
            workspace_rect: workspace,
        });
        module.update(WorkspaceIntent::UpdatePanelDrag {
            pointer: UiPoint {
                x: start.x + PANEL_DRAG_THRESHOLD - 1.0,
                y: start.y,
            },
            workspace_rect: workspace,
        });
        assert!(!module.panel_drag_is_active());
        assert!(module.update(WorkspaceIntent::CommitPanelDrag).changed);
        assert_eq!(
            module
                .active_panel_id("workspace/bottom")
                .map(PanelId::as_str),
            Some("console")
        );

        let before_cancel = module.layout().clone();
        module.update(WorkspaceIntent::BeginPanelDrag {
            panel_id: panel_id("console"),
            pointer: start,
            workspace_rect: workspace,
        });
        module.update(WorkspaceIntent::UpdatePanelDrag {
            pointer: UiPoint {
                x: bottom.x + bottom.width * 0.5,
                y: bottom.y + bottom.height * 0.5,
            },
            workspace_rect: workspace,
        });
        assert!(module.panel_drag_is_active());
        module.update(WorkspaceIntent::CancelPanelDrag);
        assert_eq!(module.layout(), &before_cancel);

        module.update(WorkspaceIntent::BeginPanelDrag {
            panel_id: panel_id("console"),
            pointer: start,
            workspace_rect: workspace,
        });
        module.update(WorkspaceIntent::UpdatePanelDrag {
            pointer: UiPoint {
                x: bottom.x + bottom.width * 0.5,
                y: bottom.y + bottom.height * 0.5,
            },
            workspace_rect: workspace,
        });
        assert!(module.update(WorkspaceIntent::CommitPanelDrag).changed);
        let mut layout = module.layout.clone();
        let DockNode::Stack { tabs, .. } =
            find_stack_mut(&mut layout.root, &layout_node_id("workspace/bottom"))
                .expect("bottom stack")
        else {
            unreachable!()
        };
        assert_eq!(tabs.last().map(PanelId::as_str), Some("console"));
    }

    #[test]
    fn workspace_docking_panel_drag_supports_all_edge_targets_and_empty_source_cleanup() {
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };
        for (zone, x_factor, y_factor) in [
            (DockDropZone::Left, 0.05, 0.5),
            (DockDropZone::Right, 0.95, 0.5),
            (DockDropZone::Top, 0.5, 0.05),
            (DockDropZone::Bottom, 0.5, 0.95),
        ] {
            let mut module = EditorWorkspaceDockingModule::standard_editor();
            let snapshot = module.snapshot(workspace);
            let source = snapshot.node_rects["workspace/left"];
            let target = snapshot.node_rects["workspace/center"];
            module.update(WorkspaceIntent::BeginPanelDrag {
                panel_id: panel_id("hierarchy"),
                pointer: UiPoint {
                    x: source.x + 12.0,
                    y: source.y + 12.0,
                },
                workspace_rect: workspace,
            });
            module.update(WorkspaceIntent::UpdatePanelDrag {
                pointer: UiPoint {
                    x: target.x + target.width * x_factor,
                    y: target.y + target.height * y_factor,
                },
                workspace_rect: workspace,
            });
            let preview = module
                .snapshot(workspace)
                .drag_preview
                .expect("edge preview");
            assert_eq!(preview.zone, zone);
            let update = module.update(WorkspaceIntent::CommitPanelDrag);
            assert!(update.changed, "{zone:?}");
            assert!(update.diagnostics.is_empty(), "{zone:?}");
            assert!(active_panel_for_stack(&module.layout().root, "workspace/left").is_none());
            assert_eq!(
                stack_id_containing(&module.layout().root, &panel_id("hierarchy"))
                    .map(LayoutNodeId::as_str),
                Some("workspace/stack/hierarchy/2")
            );
            validate_workspace_layout(module.layout(), module.registry())
                .expect("edge dock result");
        }
    }

    #[test]
    fn workspace_docking_panel_drag_stale_target_fails_closed() {
        let workspace = UiRect {
            x: 0.0,
            y: 52.0,
            width: 1280.0,
            height: 668.0,
        };
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let snapshot = module.snapshot(workspace);
        let source = snapshot.node_rects["workspace/bottom"];
        let target = snapshot.node_rects["workspace/left"];
        let original = module.layout().clone();
        module.update(WorkspaceIntent::BeginPanelDrag {
            panel_id: panel_id("ai_panel"),
            pointer: UiPoint {
                x: source.x + 12.0,
                y: source.y + 12.0,
            },
            workspace_rect: workspace,
        });
        module.update(WorkspaceIntent::UpdatePanelDrag {
            pointer: UiPoint {
                x: target.x + target.width * 0.5,
                y: target.y + target.height * 0.5,
            },
            workspace_rect: workspace,
        });
        module
            .active_panel_drag
            .as_mut()
            .unwrap()
            .target
            .as_mut()
            .unwrap()
            .node_id = layout_node_id("workspace/stale");
        let update = module.update(WorkspaceIntent::CommitPanelDrag);
        assert!(!update.changed);
        assert_eq!(update.diagnostics[0].code, "stale_dock_target");
        assert_eq!(module.layout(), &original);
    }

    #[test]
    fn workspace_layout_schema_close_show_reset_and_restore_reconcile_registry() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let original = module.layout().clone();
        let update = module.update(WorkspaceIntent::ClosePanel {
            panel_id: panel_id("hierarchy"),
        });
        assert!(!update.changed);
        assert_eq!(update.diagnostics[0].code, "panel_not_closable");
        assert_eq!(module.layout(), &original);

        assert!(
            module
                .update(WorkspaceIntent::ClosePanel {
                    panel_id: panel_id("ai_panel"),
                })
                .changed
        );
        assert!(module
            .layout()
            .closed_panels
            .contains(&panel_id("ai_panel")));
        assert!(stack_id_containing(&module.layout().root, &panel_id("ai_panel")).is_none());
        assert!(
            module
                .update(WorkspaceIntent::ShowPanel {
                    panel_id: panel_id("ai_panel"),
                })
                .changed
        );
        assert_eq!(
            stack_id_containing(&module.layout().root, &panel_id("ai_panel"))
                .map(LayoutNodeId::as_str),
            Some("workspace/right")
        );
        module.update(WorkspaceIntent::ResetLayout);
        assert_eq!(module.layout(), &original);

        let mut registry = PanelRegistry::standard_editor();
        registry.register(PanelDescriptor {
            panel_id: panel_id("new_panel"),
            title: "New".to_string(),
            minimum_size: PanelSize {
                width: 100.0,
                height: 80.0,
            },
            preferred_size: PanelSize {
                width: 240.0,
                height: 160.0,
            },
            closable: true,
            default_stack_id: "right".to_string(),
        });
        let mut persisted = original;
        if let DockNode::Split { ratio, .. } = &mut persisted.root {
            *ratio = 5.0;
        }
        let bottom = find_stack_mut(&mut persisted.root, &layout_node_id("workspace/bottom"))
            .expect("bottom stack");
        let DockNode::Stack {
            tabs,
            active_panel_id,
            ..
        } = bottom
        else {
            unreachable!()
        };
        tabs.push(panel_id("removed_panel"));
        *active_panel_id = panel_id("removed_panel");

        let (restored, report) =
            EditorWorkspaceDockingModule::restore_or_default(registry, Some(persisted));
        assert!(!report.used_default);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "removed_unknown_panel"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "split_ratio_clamped"));
        assert_eq!(
            stack_id_containing(&restored.layout().root, &panel_id("new_panel"))
                .map(LayoutNodeId::as_str),
            Some("workspace/right")
        );
        validate_workspace_layout(restored.layout(), restored.registry())
            .expect("reconciled layout");

        let registry = PanelRegistry::standard_editor();
        let mut duplicate = default_workspace_layout(&registry);
        let right = find_stack_mut(&mut duplicate.root, &layout_node_id("workspace/right"))
            .expect("right stack");
        let DockNode::Stack { tabs, .. } = right else {
            unreachable!()
        };
        tabs.push(panel_id("hierarchy"));
        let (_, report) =
            EditorWorkspaceDockingModule::restore_or_default(registry, Some(duplicate));
        assert!(report.used_default);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate_panel"));
    }

    #[test]
    fn workspace_topology_floating_roots_preserve_global_panel_ownership_and_rehome() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let placement = WorkspaceWindowPlacement {
            x: 120.0,
            y: 80.0,
            width: 640.0,
            height: 480.0,
            display_id: Some("display-1".to_string()),
        };
        let floated = module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("hierarchy"),
            window_id: WorkspaceWindowId::new("floating-1").unwrap(),
            placement: placement.clone(),
        });
        assert!(floated.changed, "{:?}", floated.diagnostics);
        let topology = module.topology();
        assert_eq!(topology.floating_roots.len(), 1);
        assert_eq!(topology.floating_roots[0].placement, placement);
        assert!(stack_id_containing(&topology.main_root.root, &panel_id("hierarchy")).is_none());

        let closed = module.update(WorkspaceIntent::CloseFloatingWindow {
            window_id: WorkspaceWindowId::new("floating-1").unwrap(),
        });
        assert!(closed.changed, "{:?}", closed.diagnostics);
        assert!(module.topology().floating_roots.is_empty());
        assert_eq!(
            stack_id_containing(&module.layout().root, &panel_id("hierarchy"))
                .map(LayoutNodeId::as_str),
            Some("workspace/left")
        );
    }

    #[test]
    fn workspace_topology_stale_cross_root_target_is_atomic() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("ai_panel"),
            window_id: WorkspaceWindowId::new("floating-1").unwrap(),
            placement: WorkspaceWindowPlacement::default(),
        });
        let before = module.topology();
        let update = module.update(WorkspaceIntent::DockPanelToWindow {
            panel_id: panel_id("ai_panel"),
            window_id: WorkspaceWindowId::main(),
            target_stack_id: layout_node_id("workspace/stale"),
            zone: DockDropZone::Center,
        });
        assert!(!update.changed);
        assert_eq!(update.diagnostics[0].code, "stale_dock_target");
        assert_eq!(module.topology(), before);
    }

    #[test]
    fn workspace_layout_v2_migrates_v1_and_clamps_invisible_placement() {
        let registry = PanelRegistry::standard_editor();
        let legacy = default_workspace_layout(&registry);
        let (mut module, restore) =
            EditorWorkspaceDockingModule::restore_topology_or_default(registry, None, Some(legacy));
        assert!(!restore.used_default);
        assert!(restore
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "migrated_layout_v1_to_v2"));
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("ai_panel"),
            window_id: WorkspaceWindowId::new("floating-1").unwrap(),
            placement: WorkspaceWindowPlacement {
                x: -50_000.0,
                y: -50_000.0,
                width: 20.0,
                height: 20.0,
                display_id: Some("missing".to_string()),
            },
        });
        let plan = module.window_plan(&[WorkspaceDisplay {
            display_id: "primary".to_string(),
            work_area: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        }]);
        let floating = plan
            .windows
            .iter()
            .find(|window| !window.window_id.is_main())
            .unwrap();
        assert!(floating.placement.x >= 0.0);
        assert!(floating.placement.y >= 0.0);
        assert!(floating.placement.width >= 240.0);
        assert!(floating.placement.height >= 180.0);
    }

    fn drag_facts(window_id: WorkspaceWindowId, x: f32, width: f32) -> WorkspaceDragWindowFacts {
        WorkspaceDragWindowFacts {
            window_id,
            screen_rect: UiRect {
                x,
                y: 0.0,
                width,
                height: 720.0,
            },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width,
                height: 720.0,
            },
            scale_factor: 1.0,
        }
    }

    #[test]
    fn workspace_cross_window_drag_moves_main_to_floating_with_one_target_token() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let floating_id = WorkspaceWindowId::new("floating-1").unwrap();
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("ai_panel"),
            window_id: floating_id.clone(),
            placement: WorkspaceWindowPlacement::default(),
        });
        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("hierarchy"),
            source_window_id: WorkspaceWindowId::main(),
            pointer: UiPoint { x: 80.0, y: 80.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 720.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint {
                x: 1450.0,
                y: 360.0,
            },
            windows: vec![
                drag_facts(WorkspaceWindowId::main(), 0.0, 1000.0),
                drag_facts(floating_id.clone(), 1200.0, 600.0),
            ],
        });
        let token = module.resolved_drag_target_token().unwrap().clone();
        assert_eq!(token.window_id, floating_id);
        assert_eq!(token.layout_revision, module.layout_revision);
        assert!(module.update(WorkspaceIntent::CommitPanelDrag).changed);
        let topology = module.topology();
        assert!(stack_id_containing(&topology.main_root.root, &panel_id("hierarchy")).is_none());
        assert!(
            stack_id_containing(&topology.floating_roots[0].root, &panel_id("hierarchy")).is_some()
        );
    }

    #[test]
    fn workspace_cross_window_drag_supports_floating_to_main_and_floating_commit() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let floating_id = WorkspaceWindowId::new("floating-1").unwrap();
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("ai_panel"),
            window_id: floating_id.clone(),
            placement: WorkspaceWindowPlacement::default(),
        });
        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("ai_panel"),
            source_window_id: floating_id.clone(),
            pointer: UiPoint { x: 80.0, y: 80.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 600.0,
                height: 720.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint { x: 500.0, y: 360.0 },
            windows: vec![
                drag_facts(WorkspaceWindowId::main(), 0.0, 1000.0),
                drag_facts(floating_id, 1200.0, 600.0),
            ],
        });
        assert!(module.update(WorkspaceIntent::CommitPanelDrag).changed);
        assert!(module.topology().floating_roots.is_empty());

        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("inspector"),
            source_window_id: WorkspaceWindowId::main(),
            pointer: UiPoint { x: 800.0, y: 100.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 720.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint {
                x: 2100.0,
                y: 400.0,
            },
            windows: vec![drag_facts(WorkspaceWindowId::main(), 0.0, 1000.0)],
        });
        assert!(module.drag_requires_native_proxy());
        let new_id = WorkspaceWindowId::new("floating-2").unwrap();
        assert!(
            module
                .update(WorkspaceIntent::CommitPanelDragToFloating {
                    window_id: new_id.clone(),
                    placement: WorkspaceWindowPlacement {
                        x: 2000.0,
                        y: 300.0,
                        ..WorkspaceWindowPlacement::default()
                    },
                })
                .changed
        );
        assert_eq!(module.topology().floating_roots[0].window_id, new_id);
    }

    #[test]
    fn workspace_cross_window_drag_moves_floating_to_floating_and_cleans_empty_source() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let first = WorkspaceWindowId::new("floating-1").unwrap();
        let second = WorkspaceWindowId::new("floating-2").unwrap();
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("ai_panel"),
            window_id: first.clone(),
            placement: WorkspaceWindowPlacement::default(),
        });
        module.update(WorkspaceIntent::FloatPanel {
            panel_id: panel_id("console"),
            window_id: second.clone(),
            placement: WorkspaceWindowPlacement::default(),
        });
        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("ai_panel"),
            source_window_id: first.clone(),
            pointer: UiPoint { x: 80.0, y: 80.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 500.0,
                height: 600.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint {
                x: 1450.0,
                y: 300.0,
            },
            windows: vec![
                drag_facts(first.clone(), 700.0, 500.0),
                drag_facts(second.clone(), 1200.0, 500.0),
            ],
        });
        assert!(module.update(WorkspaceIntent::CommitPanelDrag).changed);
        let topology = module.topology();
        assert_eq!(topology.floating_roots.len(), 1);
        assert_eq!(topology.floating_roots[0].window_id, second);
        assert!(
            stack_id_containing(&topology.floating_roots[0].root, &panel_id("ai_panel")).is_some()
        );
    }

    #[test]
    fn workspace_cross_window_drag_stale_and_cancel_preserve_topology() {
        let mut module = EditorWorkspaceDockingModule::standard_editor();
        let before = module.topology();
        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("ai_panel"),
            source_window_id: WorkspaceWindowId::main(),
            pointer: UiPoint { x: 100.0, y: 100.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 720.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint { x: 200.0, y: 200.0 },
            windows: vec![drag_facts(WorkspaceWindowId::main(), 0.0, 1000.0)],
        });
        module.layout_revision += 1;
        let stale_before = module.topology();
        let stale = module.update(WorkspaceIntent::CommitPanelDrag);
        assert_eq!(stale.diagnostics[0].code, "stale_dock_target");
        assert_eq!(module.topology(), stale_before);

        module.layout_revision -= 1;
        module.update(WorkspaceIntent::BeginPanelDragInWindow {
            panel_id: panel_id("ai_panel"),
            source_window_id: WorkspaceWindowId::main(),
            pointer: UiPoint { x: 100.0, y: 100.0 },
            workspace_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 720.0,
            },
        });
        module.update(WorkspaceIntent::UpdatePanelDragAcrossWindows {
            screen_pointer: UiPoint {
                x: 1400.0,
                y: 300.0,
            },
            windows: vec![drag_facts(WorkspaceWindowId::main(), 0.0, 1000.0)],
        });
        module.update(WorkspaceIntent::CancelPanelDrag);
        assert_eq!(module.topology(), before);
    }
}
