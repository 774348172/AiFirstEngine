use std::collections::BTreeSet;

use editor_ui_model::{
    ui_command_id_for_payload, AssetPlacementMode, AuthoringStepId, EditorCommandFeedback,
    EditorCommandFeedbackStatus, InputActionValueKind, InputMappingReportLevel, InputProcessorKind,
    InputTriggerKind, UiCommand, UiCommandPayload, UiCommandSource, Vec3,
    WorkflowCommandResolution, WorkflowCommandResolver, WorkspaceDomainKind,
};
use editor_ui_renderer::{
    hit_test, hit_test_any, pick_widget, ActivationPolicy, EditorWidgetAction, EditorWidgetNode,
    EditorWidgetTree, HitRegion, HitTarget, UiDrawList, UiPoint, WidgetId, WidgetRole,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditorInputEvent {
    PointerDown {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerUp {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerMove {
        x: f32,
        y: f32,
    },
    MouseWheel {
        delta: f32,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    FocusLost,
}

impl EditorInputEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::PointerDown { .. } => "PointerDown",
            Self::PointerUp { .. } => "PointerUp",
            Self::PointerMove { .. } => "PointerMove",
            Self::MouseWheel { .. } => "MouseWheel",
            Self::KeyDown { .. } => "KeyDown",
            Self::KeyUp { .. } => "KeyUp",
            Self::FocusLost => "FocusLost",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRouteDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputRouteResult {
    pub command: Option<UiCommand>,
    pub disabled_feedback: Option<EditorCommandFeedback>,
    pub diagnostics: Vec<InputRouteDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetInteractionSnapshot {
    pub hovered_widget_id: Option<WidgetId>,
    pub active_widget_id: Option<WidgetId>,
    pub captured_widget_id: Option<WidgetId>,
    pub focused_widget_id: Option<WidgetId>,
    pub focus_visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetInteractionUpdate {
    pub handled: bool,
    pub activation: Option<WidgetId>,
    pub disabled: Option<WidgetId>,
    pub dirty_widget_ids: Vec<WidgetId>,
    pub diagnostics: Vec<InputRouteDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorWidgetInteractionMachine {
    snapshot: WidgetInteractionSnapshot,
    keyboard_armed_key: Option<String>,
}

impl EditorWidgetInteractionMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> &WidgetInteractionSnapshot {
        &self.snapshot
    }

    pub fn set_keyboard_focus(&mut self, widget_id: Option<WidgetId>) -> Vec<WidgetId> {
        let mut dirty = BTreeSet::new();
        mark_changed(
            &mut dirty,
            self.snapshot.focused_widget_id.as_ref(),
            widget_id.as_ref(),
        );
        self.snapshot.focused_widget_id = widget_id;
        self.snapshot.focus_visible = self.snapshot.focused_widget_id.is_some();
        dirty.into_iter().collect()
    }

    pub fn handle_event(
        &mut self,
        event: &EditorInputEvent,
        tree: &EditorWidgetTree,
    ) -> WidgetInteractionUpdate {
        let before = self.snapshot.clone();
        let mut update = WidgetInteractionUpdate::default();
        match event {
            EditorInputEvent::PointerMove { x, y } => {
                let hovered = styled_control_at(tree, *x, *y).map(|node| node.id.clone());
                update.handled = hovered.is_some() || self.snapshot.captured_widget_id.is_some();
                self.snapshot.hovered_widget_id = hovered.clone();
                if let Some(captured) = self.snapshot.captured_widget_id.clone() {
                    self.snapshot.active_widget_id = tree
                        .node(&captured)
                        .filter(|node| point_inside_node(node, *x, *y))
                        .map(|node| node.id.clone());
                }
            }
            EditorInputEvent::PointerDown {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                let Some(node) = styled_control_at(tree, *x, *y) else {
                    self.clear_pointer_state();
                    return interaction_update_from_before(before, &self.snapshot, update);
                };
                update.handled = true;
                self.snapshot.hovered_widget_id = Some(node.id.clone());
                self.snapshot.focused_widget_id = Some(node.id.clone());
                self.snapshot.focus_visible = false;
                if !node.enabled {
                    self.snapshot.active_widget_id = None;
                    self.snapshot.captured_widget_id = None;
                    update.disabled = Some(node.id.clone());
                } else {
                    self.snapshot.captured_widget_id = Some(node.id.clone());
                    self.snapshot.active_widget_id = Some(node.id.clone());
                    if node.activation_policy == ActivationPolicy::Press {
                        update.activation = Some(node.id.clone());
                    }
                }
            }
            EditorInputEvent::PointerUp {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                let captured = self.snapshot.captured_widget_id.clone();
                update.handled = captured.is_some();
                if let Some(node) = captured.as_ref().and_then(|id| tree.node(id)) {
                    if node.enabled
                        && node.activation_policy == ActivationPolicy::ReleaseInside
                        && point_inside_node(node, *x, *y)
                    {
                        update.activation = Some(node.id.clone());
                    }
                }
                self.snapshot.hovered_widget_id =
                    styled_control_at(tree, *x, *y).map(|node| node.id.clone());
                self.snapshot.active_widget_id = None;
                self.snapshot.captured_widget_id = None;
            }
            EditorInputEvent::KeyDown { key }
                if matches!(key.as_str(), " " | "Space" | "Enter") =>
            {
                let Some(node) = self
                    .snapshot
                    .focused_widget_id
                    .as_ref()
                    .and_then(|id| tree.node(id))
                    .filter(|node| is_styled_control(node))
                else {
                    return interaction_update_from_before(before, &self.snapshot, update);
                };
                update.handled = true;
                self.snapshot.focus_visible = true;
                if !node.enabled {
                    update.disabled = Some(node.id.clone());
                } else if self.keyboard_armed_key.is_none() {
                    self.keyboard_armed_key = Some(key.clone());
                    self.snapshot.active_widget_id = Some(node.id.clone());
                }
            }
            EditorInputEvent::KeyUp { key }
                if self.keyboard_armed_key.as_deref() == Some(key.as_str()) =>
            {
                update.handled = true;
                self.keyboard_armed_key = None;
                if let Some(node) = self
                    .snapshot
                    .focused_widget_id
                    .as_ref()
                    .and_then(|id| tree.node(id))
                    .filter(|node| is_styled_control(node) && node.enabled)
                {
                    update.activation = Some(node.id.clone());
                }
                self.snapshot.active_widget_id = None;
            }
            EditorInputEvent::KeyDown { key } if key.eq_ignore_ascii_case("Escape") => {
                update.handled = self.snapshot.active_widget_id.is_some()
                    || self.snapshot.captured_widget_id.is_some();
                self.keyboard_armed_key = None;
                self.snapshot.active_widget_id = None;
                self.snapshot.captured_widget_id = None;
            }
            EditorInputEvent::FocusLost => {
                update.handled = self.snapshot.active_widget_id.is_some()
                    || self.snapshot.captured_widget_id.is_some();
                self.snapshot = WidgetInteractionSnapshot::default();
                self.keyboard_armed_key = None;
            }
            _ => {}
        }
        interaction_update_from_before(before, &self.snapshot, update)
    }

    fn clear_pointer_state(&mut self) {
        self.snapshot.hovered_widget_id = None;
        self.snapshot.active_widget_id = None;
        self.snapshot.captured_widget_id = None;
        self.snapshot.focused_widget_id = None;
        self.snapshot.focus_visible = false;
        self.keyboard_armed_key = None;
    }
}

fn interaction_update_from_before(
    before: WidgetInteractionSnapshot,
    after: &WidgetInteractionSnapshot,
    mut update: WidgetInteractionUpdate,
) -> WidgetInteractionUpdate {
    let mut dirty = BTreeSet::new();
    mark_changed(
        &mut dirty,
        before.hovered_widget_id.as_ref(),
        after.hovered_widget_id.as_ref(),
    );
    mark_changed(
        &mut dirty,
        before.active_widget_id.as_ref(),
        after.active_widget_id.as_ref(),
    );
    mark_changed(
        &mut dirty,
        before.focused_widget_id.as_ref(),
        after.focused_widget_id.as_ref(),
    );
    update.dirty_widget_ids = dirty.into_iter().collect();
    update
}

fn mark_changed(
    dirty: &mut BTreeSet<WidgetId>,
    before: Option<&WidgetId>,
    after: Option<&WidgetId>,
) {
    if before == after {
        return;
    }
    if let Some(before) = before {
        dirty.insert(before.clone());
    }
    if let Some(after) = after {
        dirty.insert(after.clone());
    }
}

fn styled_control_at(tree: &EditorWidgetTree, x: f32, y: f32) -> Option<&EditorWidgetNode> {
    let pick = pick_widget(tree, UiPoint { x, y }, None)?;
    tree.node(&pick.target)
        .filter(|node| is_styled_control(node))
}

fn is_styled_control(node: &EditorWidgetNode) -> bool {
    !node.control_classes.as_slice().is_empty()
        && matches!(
            node.role,
            WidgetRole::Button | WidgetRole::IconButton | WidgetRole::Tab | WidgetRole::Toggle
        )
        && node
            .binding
            .as_ref()
            .is_some_and(|binding| binding.action == EditorWidgetAction::Activate)
}

fn point_inside_node(node: &EditorWidgetNode, x: f32, y: f32) -> bool {
    let point = UiPoint { x, y };
    node.logical_rect.contains(point) && node.effective_clip.is_none_or(|clip| clip.contains(point))
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScenePickPointer {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenePickCandidate {
    pub entity_id: String,
    pub screen_x: f32,
    pub screen_y: f32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub depth_order: i32,
}

impl ScenePickCandidate {
    pub fn contains(&self, pointer: ScenePickPointer) -> bool {
        pointer.x >= self.screen_x
            && pointer.y >= self.screen_y
            && pointer.x <= self.screen_x + self.screen_width
            && pointer.y <= self.screen_y + self.screen_height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEntityPickStatus {
    Unavailable,
    NoCandidates,
    Miss,
    Hit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneEntityPickReport {
    pub pointer: ScenePickPointer,
    pub status: SceneEntityPickStatus,
    pub candidate_count: usize,
    pub selected_entity_id: Option<String>,
    pub diagnostics: Vec<String>,
}

pub const ASSET_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetDragDropTarget {
    Scene,
    InspectorField { field_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssetDragUpdate {
    None,
    Armed {
        entry_key: editor_ui_model::AssetEntryKey,
    },
    Started {
        entry_key: editor_ui_model::AssetEntryKey,
    },
    Hovering {
        entry_key: editor_ui_model::AssetEntryKey,
        target: Option<AssetDragDropTarget>,
    },
    Dropped {
        entry_key: editor_ui_model::AssetEntryKey,
        target: AssetDragDropTarget,
    },
    Cancelled {
        was_dragging: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ArmedAssetDrag {
    entry_key: editor_ui_model::AssetEntryKey,
    start_x: f32,
    start_y: f32,
    dragging: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetDragInputState {
    armed: Option<ArmedAssetDrag>,
    hover_target: Option<AssetDragDropTarget>,
}

impl AssetDragInputState {
    pub fn pointer_captured(&self) -> bool {
        self.armed.as_ref().is_some_and(|drag| drag.dragging)
    }

    pub fn hover_target(&self) -> Option<&AssetDragDropTarget> {
        self.hover_target.as_ref()
    }

    pub fn handle_event(
        &mut self,
        event: &EditorInputEvent,
        draw_list: &UiDrawList,
    ) -> AssetDragUpdate {
        match event {
            EditorInputEvent::PointerDown {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                let Some(region) = hit_test(draw_list, UiPoint { x: *x, y: *y }) else {
                    return AssetDragUpdate::None;
                };
                let HitTarget::AssetBrowserEntry { entry_key, .. } = &region.target else {
                    return AssetDragUpdate::None;
                };
                self.armed = Some(ArmedAssetDrag {
                    entry_key: entry_key.clone(),
                    start_x: *x,
                    start_y: *y,
                    dragging: false,
                });
                self.hover_target = None;
                AssetDragUpdate::Armed {
                    entry_key: entry_key.clone(),
                }
            }
            EditorInputEvent::PointerMove { x, y } => {
                let Some(armed) = self.armed.as_mut() else {
                    return AssetDragUpdate::None;
                };
                if !armed.dragging {
                    let dx = *x - armed.start_x;
                    let dy = *y - armed.start_y;
                    if dx * dx + dy * dy < ASSET_DRAG_THRESHOLD * ASSET_DRAG_THRESHOLD {
                        return AssetDragUpdate::None;
                    }
                    armed.dragging = true;
                    return AssetDragUpdate::Started {
                        entry_key: armed.entry_key.clone(),
                    };
                }
                let target = asset_drop_target_at(draw_list, *x, *y);
                self.hover_target = target.clone();
                AssetDragUpdate::Hovering {
                    entry_key: armed.entry_key.clone(),
                    target,
                }
            }
            EditorInputEvent::PointerUp {
                x,
                y,
                button: PointerButton::Primary,
            } => {
                let Some(armed) = self.armed.take() else {
                    return AssetDragUpdate::None;
                };
                let was_dragging = armed.dragging;
                let target = asset_drop_target_at(draw_list, *x, *y);
                self.hover_target = None;
                if was_dragging {
                    if let Some(target) = target {
                        AssetDragUpdate::Dropped {
                            entry_key: armed.entry_key,
                            target,
                        }
                    } else {
                        AssetDragUpdate::Cancelled { was_dragging }
                    }
                } else {
                    AssetDragUpdate::None
                }
            }
            EditorInputEvent::FocusLost => self.cancel(),
            EditorInputEvent::KeyDown { key } if key == "Escape" => self.cancel(),
            _ => AssetDragUpdate::None,
        }
    }

    fn cancel(&mut self) -> AssetDragUpdate {
        let was_dragging = self.pointer_captured();
        if self.armed.take().is_some() {
            self.hover_target = None;
            AssetDragUpdate::Cancelled { was_dragging }
        } else {
            AssetDragUpdate::None
        }
    }
}

fn asset_drop_target_at(draw_list: &UiDrawList, x: f32, y: f32) -> Option<AssetDragDropTarget> {
    let region = hit_test(draw_list, UiPoint { x, y })?;
    match &region.target {
        HitTarget::Viewport => Some(AssetDragDropTarget::Scene),
        HitTarget::InspectorField { field_id } | HitTarget::InspectorAssetPicker { field_id } => {
            Some(AssetDragDropTarget::InspectorField {
                field_id: field_id.clone(),
            })
        }
        _ => None,
    }
}

pub struct EditorInputRouter {
    request_counter: u64,
    control_down: bool,
    shift_down: bool,
}

impl Default for EditorInputRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorInputRouter {
    pub fn new() -> Self {
        Self {
            request_counter: 0,
            control_down: false,
            shift_down: false,
        }
    }

    pub fn route(&mut self, event: EditorInputEvent, draw_list: &UiDrawList) -> InputRouteResult {
        match &event {
            EditorInputEvent::KeyDown { key } if matches!(key.as_str(), "Control" | "Ctrl") => {
                self.control_down = true;
            }
            EditorInputEvent::KeyUp { key } if matches!(key.as_str(), "Control" | "Ctrl") => {
                self.control_down = false;
            }
            EditorInputEvent::KeyDown { key } if key == "Shift" => self.shift_down = true,
            EditorInputEvent::KeyUp { key } if key == "Shift" => self.shift_down = false,
            EditorInputEvent::FocusLost => {
                self.control_down = false;
                self.shift_down = false;
            }
            _ => {}
        }
        let EditorInputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        } = event
        else {
            return InputRouteResult {
                command: None,
                disabled_feedback: None,
                diagnostics: Vec::new(),
            };
        };

        if let Some(region) = hit_test_any(draw_list, UiPoint { x, y }) {
            if !region.enabled {
                return InputRouteResult {
                    command: None,
                    disabled_feedback: Some(EditorCommandFeedback {
                        command_id: region
                            .command_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        status: EditorCommandFeedbackStatus::Disabled,
                        diagnostic_code: None,
                        message: region
                            .reason_disabled
                            .clone()
                            .unwrap_or_else(|| "Command is disabled.".to_string()),
                        reason: region.reason_disabled.clone(),
                        source: UiCommandSource::Unknown,
                    }),
                    diagnostics: Vec::new(),
                };
            }
        }

        let Some(region) = hit_test(draw_list, UiPoint { x, y }) else {
            return InputRouteResult {
                command: None,
                disabled_feedback: None,
                diagnostics: vec![InputRouteDiagnostic {
                    code: "editor_input.no_hit_region".to_string(),
                    message: "Pointer event did not hit an enabled UI region.".to_string(),
                }],
            };
        };

        let command = match &region.target {
            HitTarget::ProjectLauncherAction { action_id } => {
                self.project_launcher_action_command(action_id)
            }
            HitTarget::ProjectLauncherRecentProject { project_path } => Some(self.command(
                "select_recent_project",
                UiCommandSource::ProjectLauncher,
                UiCommandPayload::SelectRecentProject {
                    path: project_path.clone(),
                },
            )),
            HitTarget::ProjectBrowserEntry { path } => Some(self.command(
                "select_project_browser_entry",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::SelectProjectBrowserEntry { path: path.clone() },
            )),
            HitTarget::ProjectBrowserOpen { path } => Some(self.command(
                "open_project_browser_entry",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::OpenProjectBrowserEntry { path: path.clone() },
            )),
            HitTarget::AssetBrowserEntry { entry_key, .. } => Some(self.command(
                "select_asset_browser_entry",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::SelectAssetBrowserEntry {
                    entry_key: entry_key.clone(),
                    additive: self.control_down,
                    range: self.shift_down,
                },
            )),
            HitTarget::AssetBrowserOpen { entry_key, .. } => Some(self.command(
                "open_asset_browser_entry",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::OpenAssetBrowserEntry {
                    entry_key: entry_key.clone(),
                },
            )),
            HitTarget::AssetBrowserFolder { path } => Some(self.command(
                "set_asset_browser_folder",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::SetAssetBrowserFolder {
                    folder: Some(path.clone()),
                },
            )),
            HitTarget::AssetBrowserAction { action } => Some(self.command(
                "asset_browser_toolbar",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::AssetBrowserToolbar { action: *action },
            )),
            HitTarget::AssetBrowserSearch => None,
            HitTarget::AiPromptField => None,
            HitTarget::DockTab { .. } => None,
            HitTarget::WorkspaceWindowMenu
            | HitTarget::WorkspacePanelVisibility { .. }
            | HitTarget::WorkspaceResetLayout => None,
            HitTarget::ToolbarOverflow => None,
            HitTarget::AssetPickerConfirm => Some(self.command(
                "confirm_asset_pick",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::ConfirmAssetPick,
            )),
            HitTarget::AssetPickerCancel => Some(self.command(
                "cancel_asset_pick",
                UiCommandSource::ProjectBrowser,
                UiCommandPayload::CancelAssetPick,
            )),
            HitTarget::AuthoringWorkflowStep { step_id } => {
                step_id.parse::<AuthoringStepId>().ok().map(|step_id| {
                    self.command(
                        "set_authoring_workflow_step",
                        UiCommandSource::Unknown,
                        UiCommandPayload::SetAuthoringWorkflowStep { step_id },
                    )
                })
            }
            HitTarget::AuthoringWorkflowCommand {
                command_id,
                payload_kind,
                domain,
            } => self.authoring_workflow_command(command_id, payload_kind, domain),
            HitTarget::ToolbarCommand { command_id } => self.toolbar_command(command_id),
            HitTarget::GameViewTarget {
                width,
                height,
                scale_policy,
            } => Some(self.command(
                "set_game_view_target",
                UiCommandSource::Viewport,
                UiCommandPayload::SetGameViewTarget {
                    width: *width,
                    height: *height,
                    scale_policy: *scale_policy,
                },
            )),
            HitTarget::HierarchyEntity { entity_id } => {
                let payload = match region.command_id.as_deref() {
                    Some("select_runtime_entity") => UiCommandPayload::SelectRuntimeEntity {
                        entity_id: entity_id.clone(),
                    },
                    Some("select_scene_entity") => UiCommandPayload::SelectSceneEntity {
                        entity_id: entity_id.clone(),
                    },
                    _ => UiCommandPayload::SelectEntity {
                        entity_id: entity_id.clone(),
                    },
                };
                Some(self.command(
                    command_id_for_payload(&payload),
                    UiCommandSource::Hierarchy,
                    payload,
                ))
            }
            HitTarget::HierarchyAction { action_id } => self.hierarchy_action_command(action_id),
            HitTarget::AuiSceneNode {
                document_path,
                document_id,
                node_id,
            } => Some(self.command(
                "select_aui_node",
                UiCommandSource::Viewport,
                UiCommandPayload::SelectAuiNode {
                    document_path: document_path.clone(),
                    document_id: document_id.clone(),
                    node_id: node_id.clone(),
                },
            )),
            HitTarget::InspectorField { .. } => None,
            HitTarget::InspectorAssetPicker { field_id } => Some(self.command(
                "begin_asset_pick",
                UiCommandSource::Inspector,
                UiCommandPayload::BeginAssetPick {
                    field_id: field_id.clone(),
                },
            )),
            HitTarget::RuntimeTraceEntry { entry_id } => Some(self.command(
                "select_trace_entry",
                UiCommandSource::RuntimeTrace,
                UiCommandPayload::SelectTraceEntry {
                    entry_id: entry_id.clone(),
                },
            )),
            HitTarget::AiProposedCommand { proposal_id } => Some(self.command(
                "ai_accept_proposed_command",
                UiCommandSource::AiAssistant,
                UiCommandPayload::AiAcceptProposedCommand {
                    proposal_id: proposal_id.clone(),
                },
            )),
            HitTarget::GatewayAccessDecision {
                request_id,
                approved,
            } => Some(self.command(
                if *approved {
                    "approve_gateway_access_request"
                } else {
                    "reject_gateway_access_request"
                },
                UiCommandSource::AiAssistant,
                if *approved {
                    UiCommandPayload::ApproveGatewayAccessRequest {
                        request_id: request_id.clone(),
                    }
                } else {
                    UiCommandPayload::RejectGatewayAccessRequest {
                        request_id: request_id.clone(),
                    }
                },
            )),
            HitTarget::ProjectRuntimeTrustDecision { request_id, action } => {
                let payload = match action.as_str() {
                    "approve" => Some(UiCommandPayload::ApproveProjectRuntimeTrust {
                        request_id: request_id.clone(),
                    }),
                    "deny" => Some(UiCommandPayload::DenyProjectRuntimeTrust {
                        request_id: request_id.clone(),
                    }),
                    "cancel" => Some(UiCommandPayload::CancelProjectRuntimeTrust {
                        request_id: request_id.clone(),
                    }),
                    _ => None,
                };
                payload.map(|payload| {
                    self.command(
                        command_id_for_payload(&payload),
                        UiCommandSource::ProjectLauncher,
                        payload,
                    )
                })
            }
            HitTarget::GatewayAccessPage { page_index } => Some(self.command(
                "set_gateway_access_page",
                UiCommandSource::AiAssistant,
                UiCommandPayload::SetGatewayAccessPage {
                    page_index: *page_index,
                },
            )),
            HitTarget::AiPanelAction { action_id } => self.ai_panel_action_command(action_id),
            HitTarget::ProjectIntentAction {
                action_id,
                subject_id,
            } => self.project_intent_action_command(action_id, subject_id),
            HitTarget::InputMappingControl {
                action,
                path,
                target_id,
                value,
            } => self.input_mapping_control_command(
                action,
                path,
                target_id.as_deref(),
                value.as_deref(),
            ),
            HitTarget::ConsoleEntry { .. }
            | HitTarget::WorkspaceSplitter { .. }
            | HitTarget::WorkspacePanelLock { .. }
            | HitTarget::WorkspacePanelMore { .. }
            | HitTarget::WorkspacePanelClose { .. }
            | HitTarget::EditorLanguageMenu
            | HitTarget::SetEditorLocale { .. }
            | HitTarget::Viewport => None,
        };

        InputRouteResult {
            command,
            disabled_feedback: None,
            diagnostics: Vec::new(),
        }
    }

    fn project_intent_action_command(
        &mut self,
        action_id: &str,
        subject_id: &str,
    ) -> Option<UiCommand> {
        let payload = match action_id {
            "park" => UiCommandPayload::ParkProjectWorkItem {
                work_item_id: subject_id.to_string(),
            },
            "resume" => UiCommandPayload::ResumeProjectWorkItem {
                work_item_id: subject_id.to_string(),
            },
            "reopen" => UiCommandPayload::ReopenProjectWorkItem {
                work_item_id: subject_id.to_string(),
            },
            "approve" => UiCommandPayload::ApproveProjectChange {
                proposal_digest: subject_id.to_string(),
            },
            "advance" => UiCommandPayload::AdvanceProjectProduction {
                run_id: subject_id.to_string(),
            },
            "cancel" => UiCommandPayload::CancelProjectProduction {
                run_id: subject_id.to_string(),
            },
            "recover" => UiCommandPayload::RecoverProjectProduction {
                run_id: subject_id.to_string(),
            },
            _ => return None,
        };
        Some(self.command(
            ui_command_id_for_payload(&payload),
            UiCommandSource::AiAssistant,
            payload,
        ))
    }

    pub fn route_widget(
        &mut self,
        event: EditorInputEvent,
        tree: &EditorWidgetTree,
        pointer_capture: Option<&WidgetId>,
    ) -> InputRouteResult {
        let EditorInputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        } = &event
        else {
            return self.route(event, &empty_draw_list());
        };
        let Some(pick) = pick_widget(tree, UiPoint { x: *x, y: *y }, pointer_capture) else {
            return self.route(event, &empty_draw_list());
        };
        let Some(node) = tree.node(&pick.target) else {
            return self.route(event, &empty_draw_list());
        };
        self.route_widget_node(node)
    }

    pub fn route_widget_activation(
        &mut self,
        widget_id: &WidgetId,
        tree: &EditorWidgetTree,
    ) -> InputRouteResult {
        let Some(node) = tree.node(widget_id) else {
            return InputRouteResult {
                command: None,
                disabled_feedback: None,
                diagnostics: vec![InputRouteDiagnostic {
                    code: "editor_input.widget_missing".to_string(),
                    message: format!("Widget {} is no longer present.", widget_id.as_str()),
                }],
            };
        };
        self.route_widget_node(node)
    }

    fn route_widget_node(&mut self, node: &EditorWidgetNode) -> InputRouteResult {
        let Some(binding) = &node.binding else {
            return InputRouteResult {
                command: None,
                disabled_feedback: None,
                diagnostics: vec![InputRouteDiagnostic {
                    code: "editor_input.widget_has_no_activation".to_string(),
                    message: format!("Picked widget {} has no command binding.", node.id.as_str()),
                }],
            };
        };
        let draw_list = UiDrawList {
            revision: 0,
            frame: 0,
            surface_width: node.logical_rect.x + node.logical_rect.width,
            surface_height: node.logical_rect.y + node.logical_rect.height,
            commands: Vec::new(),
            hit_regions: vec![HitRegion {
                id: node
                    .hit_region_id
                    .clone()
                    .unwrap_or_else(|| format!("widget.{}", node.id.as_str())),
                rect: node.logical_rect,
                target: binding.target.clone(),
                enabled: node.enabled,
                command_id: Some(binding.command_id.clone()),
                reason_disabled: binding.reason_disabled.clone(),
            }],
        };
        self.route(
            EditorInputEvent::PointerDown {
                x: node.logical_rect.x + node.logical_rect.width * 0.5,
                y: node.logical_rect.y + node.logical_rect.height * 0.5,
                button: PointerButton::Primary,
            },
            &draw_list,
        )
    }

    pub fn scene_hierarchy_select_command(&mut self, entity_id: impl Into<String>) -> UiCommand {
        let entity_id = entity_id.into();
        self.command(
            "select_scene_entity",
            UiCommandSource::Hierarchy,
            UiCommandPayload::SelectSceneEntity { entity_id },
        )
    }

    pub fn scene_toolbar_command(&mut self, command_id: &str) -> Option<UiCommand> {
        let payload = match command_id {
            "save_scene_document" | "save_scene" => {
                UiCommandPayload::SaveSceneDocument { path: None }
            }
            "open_runtime_package" => UiCommandPayload::OpenRuntimePackage {
                path: String::new(),
            },
            "undo_scene_edit" => UiCommandPayload::UndoSceneEdit,
            "redo_scene_edit" => UiCommandPayload::RedoSceneEdit,
            _ => return None,
        };
        Some(self.command(
            command_id_for_payload(&payload),
            UiCommandSource::Toolbar,
            payload,
        ))
    }

    pub fn scene_viewport_pick_command(&mut self, entity_id: impl Into<String>) -> UiCommand {
        let entity_id = entity_id.into();
        self.command(
            "select_scene_entity",
            UiCommandSource::Viewport,
            UiCommandPayload::SelectSceneEntity { entity_id },
        )
    }

    pub fn scene_viewport_pick_from_candidates(
        &mut self,
        x: f32,
        y: f32,
        candidates: &[ScenePickCandidate],
    ) -> (SceneEntityPickReport, Option<UiCommand>) {
        let pointer = ScenePickPointer { x, y };
        if candidates.is_empty() {
            return (
                SceneEntityPickReport {
                    pointer,
                    status: SceneEntityPickStatus::NoCandidates,
                    candidate_count: 0,
                    selected_entity_id: None,
                    diagnostics: vec!["scene_pick.no_candidates".to_string()],
                },
                None,
            );
        }

        let selected = candidates
            .iter()
            .filter(|candidate| candidate.contains(pointer))
            .max_by_key(|candidate| candidate.depth_order);
        let Some(selected) = selected else {
            return (
                SceneEntityPickReport {
                    pointer,
                    status: SceneEntityPickStatus::Miss,
                    candidate_count: candidates.len(),
                    selected_entity_id: None,
                    diagnostics: vec!["scene_pick.miss".to_string()],
                },
                None,
            );
        };
        let entity_id = selected.entity_id.clone();
        (
            SceneEntityPickReport {
                pointer,
                status: SceneEntityPickStatus::Hit,
                candidate_count: candidates.len(),
                selected_entity_id: Some(entity_id.clone()),
                diagnostics: Vec::new(),
            },
            Some(self.scene_viewport_pick_command(entity_id)),
        )
    }

    pub fn inspector_set_scene_transform_command(
        &mut self,
        entity_id: impl Into<String>,
        local_position: Option<Vec3>,
        local_rotation: Option<Vec3>,
        local_scale: Option<Vec3>,
    ) -> UiCommand {
        self.command(
            "set_scene_transform",
            UiCommandSource::Inspector,
            UiCommandPayload::SetSceneTransform {
                entity_id: entity_id.into(),
                local_position,
                local_rotation,
                local_scale,
            },
        )
    }

    pub fn inspector_set_scene_component_field_command(
        &mut self,
        entity_id: impl Into<String>,
        component_type: impl Into<String>,
        field_path: impl Into<String>,
        value: serde_json::Value,
    ) -> UiCommand {
        self.command(
            "set_scene_component_field",
            UiCommandSource::Inspector,
            UiCommandPayload::SetSceneComponentField {
                entity_id: entity_id.into(),
                component_type: component_type.into(),
                field_path: field_path.into(),
                value,
            },
        )
    }

    pub fn project_asset_place_into_scene_command(
        &mut self,
        asset_id: impl Into<String>,
        asset_type: impl Into<String>,
        asset_guid: Option<String>,
        target_parent_id: Option<String>,
        local_position: Option<Vec3>,
        placement_mode: AssetPlacementMode,
    ) -> UiCommand {
        self.command(
            "place_asset_into_scene",
            UiCommandSource::Viewport,
            UiCommandPayload::PlaceAssetIntoScene {
                asset_id: asset_id.into(),
                asset_type: asset_type.into(),
                asset_guid,
                target_parent_id,
                local_position,
                placement_mode,
            },
        )
    }

    pub fn ai_submit_prompt_command(&mut self, prompt: impl Into<String>) -> UiCommand {
        self.command(
            "ai_submit_prompt",
            UiCommandSource::AiAssistant,
            UiCommandPayload::AiSubmitPrompt {
                prompt: prompt.into(),
            },
        )
    }

    fn toolbar_command(&mut self, command_id: &str) -> Option<UiCommand> {
        let payload = match command_id {
            "save_scene_document" | "save_scene" => {
                UiCommandPayload::SaveSceneDocument { path: None }
            }
            "open_runtime_package" => UiCommandPayload::OpenRuntimePackage {
                path: String::new(),
            },
            "undo_scene_edit" => UiCommandPayload::UndoSceneEdit,
            "redo_scene_edit" => UiCommandPayload::RedoSceneEdit,
            "reload_runtime_package" => UiCommandPayload::ReloadRuntimePackage,
            "play" => UiCommandPayload::Play,
            "pause" => UiCommandPayload::Pause,
            "step_frame" => UiCommandPayload::StepFrame,
            "stop_play_session" => UiCommandPayload::StopPlaySession,
            "set_game_view_maximize_on_play" => {
                UiCommandPayload::SetGameViewMaximizeOnPlay { enabled: true }
            }
            "toggle_game_view_maximize_on_play" => UiCommandPayload::ToggleGameViewMaximizeOnPlay,
            "tick_one_frame" => UiCommandPayload::TickOneFrame,
            "reset_runtime" => UiCommandPayload::ResetRuntime,
            "export_desktop_package" => UiCommandPayload::ExportDesktopPackage {
                profile_id: Some("windows-dev".to_string()),
            },
            "build_and_run_desktop_package" => UiCommandPayload::BuildAndRunDesktopPackage {
                profile_id: Some("windows-dev".to_string()),
            },
            "build_release_package" => UiCommandPayload::BuildReleasePackage {
                profile_id: Some("windows-release".to_string()),
            },
            "begin_asset_pick" => UiCommandPayload::BeginAssetPick {
                field_id: "build.release.application.icon".to_string(),
            },
            "save_release_profile" => UiCommandPayload::SaveReleaseProfile,
            "open_build_output" => UiCommandPayload::OpenBuildOutput,
            "open_build_report" => UiCommandPayload::OpenBuildReport,
            "clear_console" => UiCommandPayload::ClearConsole,
            _ => return None,
        };
        Some(self.command(command_id, UiCommandSource::Toolbar, payload))
    }

    fn project_launcher_action_command(&mut self, action_id: &str) -> Option<UiCommand> {
        let payload = match action_id {
            "open_project" => UiCommandPayload::OpenProject {
                path: String::new(),
            },
            "create_project" => UiCommandPayload::CreateProject {
                path: String::new(),
                name: "NewProject".to_string(),
            },
            "create_with_ai" => UiCommandPayload::StartCreateProjectWithAi { draft_path: None },
            "refresh_recent_projects" => UiCommandPayload::RefreshRecentProjects,
            _ => return None,
        };
        Some(self.command(
            command_id_for_payload(&payload),
            UiCommandSource::ProjectLauncher,
            payload,
        ))
    }

    fn hierarchy_action_command(&mut self, action_id: &str) -> Option<UiCommand> {
        let payload = if let Some(entity_id) = action_id.strip_prefix("rename_scene_entity:") {
            UiCommandPayload::RenameSceneEntity {
                entity_id: entity_id.to_string(),
                name: "Renamed Entity".to_string(),
            }
        } else if let Some(entity_id) = action_id.strip_prefix("delete_scene_entity:") {
            UiCommandPayload::DeleteSceneEntity {
                entity_id: entity_id.to_string(),
            }
        } else {
            match action_id {
                "create_scene_entity" => UiCommandPayload::CreateSceneEntity {
                    parent_id: None,
                    name: "New Entity".to_string(),
                },
                _ => return None,
            }
        };
        Some(self.command(
            command_id_for_payload(&payload),
            UiCommandSource::Hierarchy,
            payload,
        ))
    }

    fn input_mapping_control_command(
        &mut self,
        action: &str,
        path: &str,
        target_id: Option<&str>,
        value: Option<&str>,
    ) -> Option<UiCommand> {
        let payload = match action {
            "open" => UiCommandPayload::OpenInputMapping {
                path: path.to_string(),
            },
            "validate" => UiCommandPayload::ValidateInputMapping {
                path: path.to_string(),
            },
            "save" => UiCommandPayload::SaveInputMapping {
                path: path.to_string(),
            },
            "discard" => UiCommandPayload::DiscardInputMappingDraft {
                path: path.to_string(),
            },
            "preview" => UiCommandPayload::PreviewInputMapping {
                path: path.to_string(),
                device_path: value.map(str::to_string),
            },
            "report_level" => UiCommandPayload::SetInputMappingReportLevel {
                path: path.to_string(),
                level: match value? {
                    "Off" => InputMappingReportLevel::Off,
                    "Trace" => InputMappingReportLevel::Trace,
                    _ => InputMappingReportLevel::Summary,
                },
            },
            "select_context" => UiCommandPayload::SelectInputContext {
                path: path.to_string(),
                context_id: target_id?.to_string(),
            },
            "select_action" => UiCommandPayload::SelectInputAction {
                path: path.to_string(),
                action_id: target_id?.to_string(),
            },
            "select_binding" => UiCommandPayload::SelectInputBinding {
                path: path.to_string(),
                binding_id: target_id?.to_string(),
            },
            "add_context" => UiCommandPayload::AddInputContext {
                path: path.to_string(),
                context_id: target_id?.to_string(),
                priority: value?.parse().ok()?,
            },
            "add_action" => UiCommandPayload::AddInputAction {
                path: path.to_string(),
                action_id: target_id?.to_string(),
                value_type: InputActionValueKind::Button,
            },
            "set_context_priority" => UiCommandPayload::SetInputContextPriority {
                path: path.to_string(),
                context_id: target_id?.to_string(),
                priority: value?.parse().ok()?,
            },
            "set_context_consume" => UiCommandPayload::SetInputContextConsumeInput {
                path: path.to_string(),
                context_id: target_id?.to_string(),
                consume_input: value?.parse().ok()?,
            },
            "set_action_type" => UiCommandPayload::SetInputActionValueType {
                path: path.to_string(),
                action_id: target_id?.to_string(),
                value_type: parse_input_action_value_kind(value?)?,
            },
            "add_binding" => {
                let (context_id, device_path) = value?.split_once('|')?;
                UiCommandPayload::AddInputBinding {
                    path: path.to_string(),
                    context_id: context_id.to_string(),
                    action_id: target_id?.to_string(),
                    device_path: device_path.to_string(),
                }
            }
            "set_device_path" => UiCommandPayload::SetInputBindingDevicePathById {
                path: path.to_string(),
                binding_id: target_id?.to_string(),
                device_path: value?.to_string(),
            },
            "set_trigger" => UiCommandPayload::SetInputBindingTrigger {
                path: path.to_string(),
                binding_id: target_id?.to_string(),
                trigger: match value? {
                    "Down" => InputTriggerKind::Down,
                    "Released" => InputTriggerKind::Released,
                    _ => InputTriggerKind::Pressed,
                },
            },
            "set_processor" => UiCommandPayload::SetInputBindingProcessor {
                path: path.to_string(),
                binding_id: target_id?.to_string(),
                processor: match value? {
                    "Invert" => InputProcessorKind::Invert,
                    _ => InputProcessorKind::None,
                },
            },
            "begin_capture" => UiCommandPayload::BeginInputBindingCapture {
                path: path.to_string(),
                binding_id: target_id?.to_string(),
            },
            "cancel_capture" => UiCommandPayload::CancelInputBindingCapture {
                path: path.to_string(),
            },
            _ => return None,
        };
        Some(self.command(
            ui_command_id_for_payload(&payload),
            UiCommandSource::Unknown,
            payload,
        ))
    }

    fn ai_panel_action_command(&mut self, action_id: &str) -> Option<UiCommand> {
        if let Some(prompt) = action_id.strip_prefix("submit:") {
            let payload = UiCommandPayload::GenerateProjectPatchFromPrompt {
                prompt: prompt.to_string(),
            };
            return Some(self.command(
                ui_command_id_for_payload(&payload),
                UiCommandSource::AiAssistant,
                payload,
            ));
        }
        if action_id == "cancel" {
            let payload = UiCommandPayload::CancelLlmPatchRequest;
            return Some(self.command(
                ui_command_id_for_payload(&payload),
                UiCommandSource::AiAssistant,
                payload,
            ));
        }
        if let Some(proposal_id) = action_id.strip_prefix("reject:") {
            return Some(self.command(
                "ai_reject_proposed_command",
                UiCommandSource::AiAssistant,
                UiCommandPayload::AiRejectProposedCommand {
                    proposal_id: proposal_id.to_string(),
                },
            ));
        }
        None
    }

    fn authoring_workflow_command(
        &mut self,
        command_id: &str,
        payload_kind: &str,
        domain: &str,
    ) -> Option<UiCommand> {
        let domain = parse_workspace_domain(domain)?;
        match WorkflowCommandResolver::resolve_parts(
            command_id,
            payload_kind,
            domain,
            editor_ui_model::AuthoringCommandAvailability::Available,
            command_id,
        ) {
            WorkflowCommandResolution::Command(payload) => Some(self.command(
                command_id_for_payload(&payload),
                UiCommandSource::Unknown,
                payload,
            )),
            WorkflowCommandResolution::FocusDomainPanel { .. }
            | WorkflowCommandResolution::Disabled { .. }
            | WorkflowCommandResolution::Unsupported { .. } => None,
        }
    }

    fn command(
        &mut self,
        command_id: &str,
        source: UiCommandSource,
        payload: UiCommandPayload,
    ) -> UiCommand {
        self.request_counter += 1;
        UiCommand {
            command_id: command_id.to_string(),
            source,
            request_id: format!("input-request-{}", self.request_counter),
            payload,
        }
    }
}

fn empty_draw_list() -> UiDrawList {
    UiDrawList {
        revision: 0,
        frame: 0,
        surface_width: 0.0,
        surface_height: 0.0,
        commands: Vec::new(),
        hit_regions: Vec::new(),
    }
}

fn parse_input_action_value_kind(value: &str) -> Option<InputActionValueKind> {
    match value {
        "Button" => Some(InputActionValueKind::Button),
        "Axis1" => Some(InputActionValueKind::Axis1),
        "Axis2" => Some(InputActionValueKind::Axis2),
        "Pointer" => Some(InputActionValueKind::Pointer),
        _ => None,
    }
}

fn command_id_for_payload(payload: &UiCommandPayload) -> &'static str {
    ui_command_id_for_payload(payload)
}

fn parse_workspace_domain(value: &str) -> Option<WorkspaceDomainKind> {
    match value {
        "project" => Some(WorkspaceDomainKind::Project),
        "scene" => Some(WorkspaceDomainKind::Scene),
        "asset" => Some(WorkspaceDomainKind::Asset),
        "prefab" => Some(WorkspaceDomainKind::Prefab),
        "rule" => Some(WorkspaceDomainKind::Rule),
        "aui" => Some(WorkspaceDomainKind::Aui),
        "input" => Some(WorkspaceDomainKind::Input),
        "play" => Some(WorkspaceDomainKind::Play),
        "build" => Some(WorkspaceDomainKind::Build),
        "report" => Some(WorkspaceDomainKind::Report),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_model::{
        AiCommandReviewState, AiPanelMessage, AiPanelMessageRole, AiPanelModel, AiProposedCommand,
        Animator2DAuthoringModel, BuildExportCommand, BuildExportModel, BuildProfileSummary,
        ConsoleModel, EditorUiMode, EditorUiModel, GatewayAccessInboxModel,
        GatewayAccessRequestModel, HierarchyModel, HierarchyNode, InspectorField, InspectorModel,
        InspectorSection, InspectorValue, InspectorValueType, PanelLayoutModel,
        ProjectBrowserEntry, ProjectBrowserEntryKind, ProjectBrowserModel, ProjectLauncherModel,
        RecentProjectEntry, RuntimeRunState, RuntimeTraceModel, ToolbarCommand, ToolbarModel,
        UiCommandPayload, Vec3, ViewportModel, WorkspaceViewMode,
    };
    use editor_ui_renderer::{HitTarget, SelfUiRenderer, UiRendererConfig};

    #[test]
    fn router_maps_toolbar_hit_to_ui_command() {
        let model = fixture_model();
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("asset_browser".to_string())),
        );
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| {
                region.target
                    == HitTarget::ToolbarCommand {
                        command_id: "tick_one_frame".to_string(),
                    }
            })
            .expect("tick hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );
        let command = result.command.expect("toolbar should produce command");
        assert_eq!(command.source, UiCommandSource::Toolbar);
        assert_eq!(command.payload, UiCommandPayload::TickOneFrame);
    }

    #[test]
    fn game_view_target_hit_routes_typed_session_command() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![editor_ui_renderer::HitRegion {
                id: "hit.viewport.game_view_target.720x1280.contain".to_string(),
                rect: editor_ui_renderer::UiRect {
                    x: 10.0,
                    y: 10.0,
                    width: 80.0,
                    height: 24.0,
                },
                target: HitTarget::GameViewTarget {
                    width: 720,
                    height: 1280,
                    scale_policy: editor_ui_model::EditorGameViewScalePolicy::Contain,
                },
                enabled: true,
                command_id: Some("set_game_view_target".to_string()),
                reason_disabled: None,
            }],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 12.0,
                y: 12.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        let command = result.command.expect("target control should route");
        assert_eq!(command.source, UiCommandSource::Viewport);
        assert_eq!(
            command.payload,
            UiCommandPayload::SetGameViewTarget {
                width: 720,
                height: 1280,
                scale_policy: editor_ui_model::EditorGameViewScalePolicy::Contain,
            }
        );
    }

    #[test]
    fn input_mapping_hit_routes_stable_binding_command() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![editor_ui_renderer::HitRegion {
                id: "hit.input.device.binding.fire.mouse_Left".to_string(),
                rect: editor_ui_renderer::UiRect {
                    x: 10.0,
                    y: 10.0,
                    width: 120.0,
                    height: 20.0,
                },
                target: HitTarget::InputMappingControl {
                    action: "set_device_path".to_string(),
                    path: "Input/input.default.json".to_string(),
                    target_id: Some("binding.fire".to_string()),
                    value: Some("mouse/Left".to_string()),
                },
                enabled: true,
                command_id: Some("set_input_binding_device_path_by_id".to_string()),
                reason_disabled: None,
            }],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 12.0,
                y: 12.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result.command.unwrap().payload,
            UiCommandPayload::SetInputBindingDevicePathById {
                path: "Input/input.default.json".to_string(),
                binding_id: "binding.fire".to_string(),
                device_path: "mouse/Left".to_string(),
            }
        );
    }

    #[test]
    fn router_returns_disabled_feedback_for_disabled_toolbar_hit() {
        let mut model = fixture_model();
        model.toolbar.commands[0].enabled = false;
        model.toolbar.commands[0].reason_disabled =
            Some("Open a Runtime Package first.".to_string());
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("asset_browser".to_string())),
        );
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.toolbar.tick_one_frame")
            .expect("disabled toolbar hit region");
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        let feedback = result
            .disabled_feedback
            .expect("disabled hit should produce feedback");
        assert!(result.command.is_none());
        assert_eq!(feedback.command_id, "tick_one_frame");
        assert_eq!(feedback.status, EditorCommandFeedbackStatus::Disabled);
        assert_eq!(
            feedback.reason.as_deref(),
            Some("Open a Runtime Package first.")
        );
    }

    #[test]
    fn router_maps_hierarchy_hit_to_select_scene_entity_from_authoring_draw_list() {
        let model = fixture_model();
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.hierarchy.entity-player")
            .expect("hierarchy hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );
        assert_eq!(
            result
                .command
                .expect("hierarchy should produce command")
                .payload,
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-player".to_string()
            }
        );
    }

    #[test]
    fn router_maps_runtime_hierarchy_hit_to_select_runtime_entity() {
        let mut model = fixture_model();
        model.hierarchy.source_domain =
            editor_ui_model::HierarchySourceDomain::ActiveGameViewRuntime;
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.hierarchy.entity-player")
            .expect("hierarchy hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );
        assert_eq!(
            result
                .command
                .expect("runtime hierarchy should produce command")
                .payload,
            UiCommandPayload::SelectRuntimeEntity {
                entity_id: "entity-player".to_string()
            }
        );
    }

    #[test]
    fn router_maps_project_launcher_open_hit_to_command() {
        let mut model = fixture_model();
        model.mode = EditorUiMode::ProjectLauncher;
        model.project_launcher = ProjectLauncherModel::empty();
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.project_launcher.open_project")
            .expect("open project hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        let command = result.command.expect("launcher hit should route");
        assert_eq!(command.source, UiCommandSource::ProjectLauncher);
        assert_eq!(
            command.payload,
            UiCommandPayload::OpenProject {
                path: String::new()
            }
        );
    }

    #[test]
    fn router_maps_project_launcher_recent_project_to_command() {
        let mut model = fixture_model();
        model.mode = EditorUiMode::ProjectLauncher;
        model.project_launcher = ProjectLauncherModel::empty();
        model
            .project_launcher
            .recent_projects
            .push(RecentProjectEntry {
                name: "PlaneGame".to_string(),
                path: "D:/Projects/PlaneGame".to_string(),
                engine_version: "0.0.1".to_string(),
                last_opened_at: None,
                last_modified_at: None,
                valid: true,
                status: "ready".to_string(),
            });
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.project_launcher.recent.0")
            .expect("recent project hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result.command.expect("recent project should route").payload,
            UiCommandPayload::SelectRecentProject {
                path: "D:/Projects/PlaneGame".to_string()
            }
        );
    }

    #[test]
    fn router_maps_asset_browser_row_to_stable_select_command_with_modifier() {
        let mut model = fixture_model();
        let entry = editor_ui_model::AssetBrowserEntry::authoring(
            "Scenes/Main.scene.json",
            "Main.scene.json",
            editor_ui_model::AssetKind::Scene,
            editor_ui_model::EditorAssetRef::new("scene-main", "scene"),
        );
        model.asset_browser.index_status = editor_ui_model::AssetBrowserIndexStatus::Ready;
        model.asset_browser.entries = vec![entry.clone()];
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("asset_browser".to_string())),
        );
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| matches!(region.target, HitTarget::AssetBrowserEntry { .. }))
            .expect("asset browser row hit region");
        let mut router = EditorInputRouter::new();
        let _ = router.route(
            EditorInputEvent::KeyDown {
                key: "Control".to_string(),
            },
            &draw_list,
        );
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result
                .command
                .expect("asset browser row should route")
                .payload,
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key: entry.entry_key,
                additive: true,
                range: false,
            }
        );
    }

    #[test]
    fn router_maps_asset_browser_open_button_to_stable_open_command() {
        let mut model = fixture_model();
        let entry = editor_ui_model::AssetBrowserEntry::authoring(
            "Scenes/Main.scene.json",
            "Main.scene.json",
            editor_ui_model::AssetKind::Scene,
            editor_ui_model::EditorAssetRef::new("scene-main", "scene"),
        );
        model.asset_browser.index_status = editor_ui_model::AssetBrowserIndexStatus::Ready;
        model.asset_browser.entries = vec![entry.clone()];
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("asset_browser".to_string())),
        );
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| matches!(region.target, HitTarget::AssetBrowserOpen { .. }))
            .expect("asset browser open hit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result
                .command
                .expect("asset browser open should route")
                .payload,
            UiCommandPayload::OpenAssetBrowserEntry {
                entry_key: entry.entry_key,
            }
        );
    }

    #[test]
    fn asset_browser_drag_respects_threshold_captures_and_drops_on_scene() {
        let (draw_list, entry_key) = asset_drag_draw_list();
        let mut drag = AssetDragInputState::default();

        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerDown {
                    x: 10.0,
                    y: 10.0,
                    button: PointerButton::Primary,
                },
                &draw_list,
            ),
            AssetDragUpdate::Armed {
                entry_key: entry_key.clone(),
            }
        );
        assert!(!drag.pointer_captured());
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerMove { x: 15.9, y: 10.0 },
                &draw_list,
            ),
            AssetDragUpdate::None
        );
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerMove { x: 16.0, y: 10.0 },
                &draw_list,
            ),
            AssetDragUpdate::Started {
                entry_key: entry_key.clone(),
            }
        );
        assert!(drag.pointer_captured());
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerMove { x: 120.0, y: 20.0 },
                &draw_list,
            ),
            AssetDragUpdate::Hovering {
                entry_key: entry_key.clone(),
                target: Some(AssetDragDropTarget::Scene),
            }
        );
        assert_eq!(drag.hover_target(), Some(&AssetDragDropTarget::Scene));
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerUp {
                    x: 120.0,
                    y: 20.0,
                    button: PointerButton::Primary,
                },
                &draw_list,
            ),
            AssetDragUpdate::Dropped {
                entry_key,
                target: AssetDragDropTarget::Scene,
            }
        );
        assert!(!drag.pointer_captured());
        assert!(drag.hover_target().is_none());
    }

    #[test]
    fn asset_browser_drag_targets_inspector_and_escape_cancels_capture() {
        let (draw_list, entry_key) = asset_drag_draw_list();
        let mut drag = AssetDragInputState::default();
        let inspector_target = AssetDragDropTarget::InspectorField {
            field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
        };

        let _ = drag.handle_event(
            &EditorInputEvent::PointerDown {
                x: 10.0,
                y: 10.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerMove { x: 40.0, y: 10.0 },
                &draw_list,
            ),
            AssetDragUpdate::Started {
                entry_key: entry_key.clone(),
            }
        );
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::PointerMove { x: 230.0, y: 20.0 },
                &draw_list,
            ),
            AssetDragUpdate::Hovering {
                entry_key,
                target: Some(inspector_target.clone()),
            }
        );
        assert_eq!(drag.hover_target(), Some(&inspector_target));
        assert_eq!(
            drag.handle_event(
                &EditorInputEvent::KeyDown {
                    key: "Escape".to_string(),
                },
                &draw_list,
            ),
            AssetDragUpdate::Cancelled { was_dragging: true }
        );
        assert!(!drag.pointer_captured());
        assert!(drag.hover_target().is_none());
    }

    fn asset_drag_draw_list() -> (UiDrawList, editor_ui_model::AssetEntryKey) {
        let entry_key = editor_ui_model::AssetEntryKey::authoring(
            &editor_ui_model::EditorAssetRef::new("texture-icon", "texture"),
        );
        let region =
            |id: &str, x: f32, width: f32, target: HitTarget| -> editor_ui_renderer::HitRegion {
                editor_ui_renderer::HitRegion {
                    id: id.to_string(),
                    rect: editor_ui_renderer::UiRect {
                        x,
                        y: 0.0,
                        width,
                        height: 100.0,
                    },
                    target,
                    enabled: true,
                    command_id: None,
                    reason_disabled: None,
                }
            };
        (
            UiDrawList {
                revision: 1,
                frame: 0,
                surface_width: 320.0,
                surface_height: 100.0,
                commands: Vec::new(),
                hit_regions: vec![
                    region(
                        "hit.asset",
                        0.0,
                        30.0,
                        HitTarget::AssetBrowserEntry {
                            entry_key: entry_key.clone(),
                            path: "Assets/icon.asset".to_string(),
                        },
                    ),
                    region("hit.viewport", 100.0, 80.0, HitTarget::Viewport),
                    region(
                        "hit.inspector",
                        210.0,
                        100.0,
                        HitTarget::InspectorAssetPicker {
                            field_id: "components.SpriteRenderer2D.spriteRef".to_string(),
                        },
                    ),
                ],
            },
            entry_key,
        )
    }

    #[test]
    fn router_maps_hierarchy_hit_to_select_scene_entity() {
        let mut router = EditorInputRouter::new();

        let command = router.scene_hierarchy_select_command("entity-player");

        assert_eq!(command.source, UiCommandSource::Hierarchy);
        assert_eq!(
            command.payload,
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-player".to_string()
            }
        );
    }

    #[test]
    fn router_maps_toolbar_save_to_save_scene_document() {
        let mut router = EditorInputRouter::new();

        let command = router
            .scene_toolbar_command("save_scene")
            .expect("save scene command");

        assert_eq!(command.source, UiCommandSource::Toolbar);
        assert_eq!(
            command.payload,
            UiCommandPayload::SaveSceneDocument { path: None }
        );
    }

    #[test]
    fn router_maps_toolbar_open_runtime_package_to_folder_request_command() {
        let mut router = EditorInputRouter::new();

        let command = router
            .scene_toolbar_command("open_runtime_package")
            .expect("open runtime package command");

        assert_eq!(command.source, UiCommandSource::Toolbar);
        assert_eq!(
            command.payload,
            UiCommandPayload::OpenRuntimePackage {
                path: String::new()
            }
        );
    }

    #[test]
    fn router_maps_build_export_toolbar_commands() {
        let model = fixture_model();
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("build_export".to_string())),
        );

        for (command_id, expected_payload) in [
            (
                "export_desktop_package",
                UiCommandPayload::ExportDesktopPackage {
                    profile_id: Some("windows-dev".to_string()),
                },
            ),
            (
                "build_and_run_desktop_package",
                UiCommandPayload::BuildAndRunDesktopPackage {
                    profile_id: Some("windows-dev".to_string()),
                },
            ),
            (
                "build_release_package",
                UiCommandPayload::BuildReleasePackage {
                    profile_id: Some("windows-release".to_string()),
                },
            ),
            (
                "begin_asset_pick",
                UiCommandPayload::BeginAssetPick {
                    field_id: "build.release.application.icon".to_string(),
                },
            ),
            ("save_release_profile", UiCommandPayload::SaveReleaseProfile),
            ("open_build_output", UiCommandPayload::OpenBuildOutput),
            ("open_build_report", UiCommandPayload::OpenBuildReport),
        ] {
            let region = draw_list
                .hit_regions
                .iter()
                .find(|region| region.id == format!("hit.build_export.{command_id}"))
                .expect("build export hit region");
            let mut router = EditorInputRouter::new();
            let routed = router.route(
                EditorInputEvent::PointerDown {
                    x: region.rect.x + 1.0,
                    y: region.rect.y + 1.0,
                    button: PointerButton::Primary,
                },
                &draw_list,
            );
            let command = routed.command.unwrap_or_else(|| {
                panic!(
                    "build export command {command_id} was not routed: {:?}",
                    routed.diagnostics
                )
            });

            assert_eq!(command.command_id, command_id);
            assert_eq!(command.source, UiCommandSource::Toolbar);
            assert_eq!(command.payload, expected_payload);
        }
    }

    #[test]
    fn router_maps_toolbar_undo_to_undo_scene_edit() {
        let mut router = EditorInputRouter::new();

        let command = router
            .scene_toolbar_command("undo_scene_edit")
            .expect("undo scene command");

        assert_eq!(command.payload, UiCommandPayload::UndoSceneEdit);
    }

    #[test]
    fn router_maps_toolbar_redo_to_redo_scene_edit() {
        let mut router = EditorInputRouter::new();

        let command = router
            .scene_toolbar_command("redo_scene_edit")
            .expect("redo scene command");

        assert_eq!(command.payload, UiCommandPayload::RedoSceneEdit);
    }

    #[test]
    fn router_maps_viewport_pick_to_select_scene_entity() {
        let mut router = EditorInputRouter::new();

        let command = router.scene_viewport_pick_command("entity-player");

        assert_eq!(command.source, UiCommandSource::Viewport);
        assert_eq!(
            command.payload,
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-player".to_string()
            }
        );
    }

    #[test]
    fn scene_pick_reports_hit_and_emits_viewport_select_command() {
        let mut router = EditorInputRouter::new();
        let candidates = vec![
            ScenePickCandidate {
                entity_id: "entity-back".to_string(),
                screen_x: 0.0,
                screen_y: 0.0,
                screen_width: 100.0,
                screen_height: 100.0,
                depth_order: 1,
            },
            ScenePickCandidate {
                entity_id: "entity-front".to_string(),
                screen_x: 0.0,
                screen_y: 0.0,
                screen_width: 100.0,
                screen_height: 100.0,
                depth_order: 10,
            },
        ];

        let (report, command) = router.scene_viewport_pick_from_candidates(50.0, 50.0, &candidates);

        assert_eq!(report.status, SceneEntityPickStatus::Hit);
        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.selected_entity_id.as_deref(), Some("entity-front"));
        assert_eq!(
            command.expect("hit should emit command").payload,
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-front".to_string()
            }
        );
    }

    #[test]
    fn scene_pick_reports_no_candidates_without_fabricating_selection() {
        let mut router = EditorInputRouter::new();

        let (report, command) = router.scene_viewport_pick_from_candidates(50.0, 50.0, &[]);

        assert_eq!(report.status, SceneEntityPickStatus::NoCandidates);
        assert_eq!(report.candidate_count, 0);
        assert!(report.selected_entity_id.is_none());
        assert!(command.is_none());
        assert!(report
            .diagnostics
            .contains(&"scene_pick.no_candidates".to_string()));
    }

    #[test]
    fn aui_scene_hit_region_routes_to_select_aui_node_before_viewport() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![
                editor_ui_renderer::HitRegion {
                    id: "hit.viewport".to_string(),
                    rect: editor_ui_renderer::UiRect {
                        x: 0.0,
                        y: 0.0,
                        width: 320.0,
                        height: 200.0,
                    },
                    target: HitTarget::Viewport,
                    enabled: true,
                    command_id: None,
                    reason_disabled: None,
                },
                editor_ui_renderer::HitRegion {
                    id: "hit.aui.score_text".to_string(),
                    rect: editor_ui_renderer::UiRect {
                        x: 16.0,
                        y: 16.0,
                        width: 220.0,
                        height: 40.0,
                    },
                    target: HitTarget::AuiSceneNode {
                        document_path: "AUI/hud.aui.json".to_string(),
                        document_id: "hud".to_string(),
                        node_id: "score_text".to_string(),
                    },
                    enabled: true,
                    command_id: Some("select_aui_node".to_string()),
                    reason_disabled: None,
                },
            ],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 32.0,
                y: 24.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result.command.expect("AUI hit should route").payload,
            UiCommandPayload::SelectAuiNode {
                document_path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                node_id: "score_text".to_string()
            }
        );
    }

    #[test]
    fn inspector_vec3_x_change_emits_set_scene_transform() {
        let mut router = EditorInputRouter::new();

        let command = router.inspector_set_scene_transform_command(
            "entity-player",
            Some(Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            }),
            None,
            None,
        );

        assert_eq!(command.source, UiCommandSource::Inspector);
        assert_eq!(
            command.payload,
            UiCommandPayload::SetSceneTransform {
                entity_id: "entity-player".to_string(),
                local_position: Some(Vec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0
                }),
                local_rotation: None,
                local_scale: None
            }
        );
    }

    #[test]
    fn inspector_component_field_change_emits_set_scene_component_field() {
        let mut router = EditorInputRouter::new();

        let command = router.inspector_set_scene_component_field_command(
            "entity-player",
            "game.health",
            "hp",
            serde_json::json!(8),
        );

        assert_eq!(command.source, UiCommandSource::Inspector);
        assert_eq!(
            command.payload,
            UiCommandPayload::SetSceneComponentField {
                entity_id: "entity-player".to_string(),
                component_type: "game.health".to_string(),
                field_path: "hp".to_string(),
                value: serde_json::json!(8)
            }
        );
    }

    #[test]
    fn project_asset_action_emits_place_asset_into_scene() {
        let mut router = EditorInputRouter::new();

        let command = router.project_asset_place_into_scene_command(
            "model-enemy",
            "model",
            Some("guid-model-enemy".to_string()),
            None,
            Some(Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            AssetPlacementMode::WorldOrigin,
        );

        assert_eq!(command.source, UiCommandSource::Viewport);
        assert_eq!(
            command.payload,
            UiCommandPayload::PlaceAssetIntoScene {
                asset_id: "model-enemy".to_string(),
                asset_type: "model".to_string(),
                asset_guid: Some("guid-model-enemy".to_string()),
                target_parent_id: None,
                local_position: Some(Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0
                }),
                placement_mode: AssetPlacementMode::WorldOrigin,
            }
        );
    }

    #[test]
    fn router_maps_ai_proposal_hit_to_accept_command() {
        let model = fixture_model();
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("ai_panel".to_string())),
        );
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.ai_proposal.accept.proposal-1")
            .expect("ai proposal accept region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result
                .command
                .expect("ai proposal should produce command")
                .payload,
            UiCommandPayload::AiAcceptProposedCommand {
                proposal_id: "proposal-1".to_string()
            }
        );
    }

    #[test]
    fn gateway_access_hits_route_to_request_bound_commands() {
        let mut model = fixture_model();
        model.ai_panel.gateway_access = GatewayAccessInboxModel {
            requests: vec![GatewayAccessRequestModel {
                request_id: "access-request-1".to_string(),
                operation_short_id: "operation-1".to_string(),
                client_session_id: "gateway-session-1".to_string(),
                session_short_id: "session-1".to_string(),
                client_kind: "MCP".to_string(),
                client_version: "codex-desktop.v1".to_string(),
                project_identity: "project.fixture".to_string(),
                connected_age_ms: 100,
                expires_in_ms: 10_000,
                state: "awaiting_user".to_string(),
                requested_profile: "project_owned_low_risk".to_string(),
                risk_class: "ProjectOwnedLowRisk".to_string(),
                capabilities: vec!["mutate_project".to_string()],
                blocked_capabilities: Vec::new(),
                goal_id: "goal-1".to_string(),
                user_visible_outcome: "Apply the requested project change.".to_string(),
                completion_policy: "CommitVerified".to_string(),
                allowed_paths: vec!["Assets".to_string()],
                denied_paths: vec!["Engine".to_string()],
                allowed_objects: Vec::new(),
                max_mutation_count: 16,
                time_budget_ms: 900_000,
                external_cost_budget_microunits: 0,
                allow_delete: false,
                allow_dependency_change: false,
                allow_network: false,
                approval_digest: "sha256:test".to_string(),
            }],
            page_index: 0,
            page_count: 2,
            total_count: 5,
        };
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("ai_panel".to_string())),
        );
        let mut router = EditorInputRouter::new();

        for (region_id, expected) in [
            (
                "hit.gateway_access.approve.access-request-1",
                UiCommandPayload::ApproveGatewayAccessRequest {
                    request_id: "access-request-1".to_string(),
                },
            ),
            (
                "hit.gateway_access.page.next.1",
                UiCommandPayload::SetGatewayAccessPage { page_index: 1 },
            ),
        ] {
            let region = draw_list
                .hit_regions
                .iter()
                .find(|region| region.id == region_id)
                .unwrap_or_else(|| panic!("missing Gateway access hit region {region_id}"));
            let result = router.route(
                EditorInputEvent::PointerDown {
                    x: region.rect.x + 1.0,
                    y: region.rect.y + 1.0,
                    button: PointerButton::Primary,
                },
                &draw_list,
            );
            assert_eq!(
                result.command.expect("Gateway hit should route").payload,
                expected
            );
        }
    }

    #[test]
    fn router_maps_project_intent_actions_to_bound_workflow_payloads() {
        let mut router = EditorInputRouter::new();
        for (action_id, subject_id, expected) in [
            (
                "park",
                "work-1",
                UiCommandPayload::ParkProjectWorkItem {
                    work_item_id: "work-1".to_string(),
                },
            ),
            (
                "approve",
                "sha256:proposal",
                UiCommandPayload::ApproveProjectChange {
                    proposal_digest: "sha256:proposal".to_string(),
                },
            ),
            (
                "advance",
                "run-1",
                UiCommandPayload::AdvanceProjectProduction {
                    run_id: "run-1".to_string(),
                },
            ),
        ] {
            let draw_list = UiDrawList {
                revision: 1,
                frame: 1,
                surface_width: 100.0,
                surface_height: 24.0,
                commands: Vec::new(),
                hit_regions: vec![HitRegion {
                    id: format!("hit.intent.{action_id}"),
                    rect: editor_ui_renderer::UiRect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 24.0,
                    },
                    target: HitTarget::ProjectIntentAction {
                        action_id: action_id.to_string(),
                        subject_id: subject_id.to_string(),
                    },
                    enabled: true,
                    command_id: Some(action_id.to_string()),
                    reason_disabled: None,
                }],
            };
            let result = router.route(
                EditorInputEvent::PointerDown {
                    x: 1.0,
                    y: 1.0,
                    button: PointerButton::Primary,
                },
                &draw_list,
            );
            assert_eq!(result.command.expect("intent command").payload, expected);
        }
    }

    #[test]
    fn ai_panel_submit_routes_editable_prompt_to_project_patch_generation() {
        let model = fixture_model();
        let mut draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(1280.0, 720.0)
                .with_active_bottom_panel(Some("ai_panel".to_string())),
        );
        draw_list
            .hit_regions
            .retain(|region| matches!(region.target, HitTarget::AiPanelAction { .. }));
        let region = draw_list.hit_regions.first().expect("AI submit region");
        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result.command.unwrap().payload,
            UiCommandPayload::GenerateProjectPatchFromPrompt {
                prompt: "create an entity".to_string()
            }
        );
    }

    #[test]
    fn router_helper_emits_ai_submit_prompt_command() {
        let mut router = EditorInputRouter::new();

        let command = router.ai_submit_prompt_command("create entity");

        assert_eq!(command.source, UiCommandSource::AiAssistant);
        assert_eq!(
            command.payload,
            UiCommandPayload::AiSubmitPrompt {
                prompt: "create entity".to_string()
            }
        );
    }

    #[test]
    fn router_maps_authoring_workflow_step_hit_to_ui_command() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![editor_ui_renderer::HitRegion {
                id: "hit.authoring_workflow_step.build".to_string(),
                rect: editor_ui_renderer::UiRect {
                    x: 10.0,
                    y: 10.0,
                    width: 100.0,
                    height: 24.0,
                },
                target: HitTarget::AuthoringWorkflowStep {
                    step_id: "build".to_string(),
                },
                enabled: true,
                command_id: Some("set_authoring_workflow_step".to_string()),
                reason_disabled: None,
            }],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 12.0,
                y: 12.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result.command.expect("workflow hit should route").payload,
            UiCommandPayload::SetAuthoringWorkflowStep {
                step_id: AuthoringStepId::Build
            }
        );
    }

    #[test]
    fn router_maps_authoring_workflow_command_hit_through_resolver() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![editor_ui_renderer::HitRegion {
                id: "hit.authoring_workflow_command.build.primary".to_string(),
                rect: editor_ui_renderer::UiRect {
                    x: 10.0,
                    y: 10.0,
                    width: 100.0,
                    height: 24.0,
                },
                target: HitTarget::AuthoringWorkflowCommand {
                    command_id: "export_desktop_package".to_string(),
                    payload_kind: "ExportDesktopPackage".to_string(),
                    domain: "build".to_string(),
                },
                enabled: true,
                command_id: Some("export_desktop_package".to_string()),
                reason_disabled: None,
            }],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 12.0,
                y: 12.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert_eq!(
            result
                .command
                .expect("workflow command should route")
                .payload,
            UiCommandPayload::ExportDesktopPackage { profile_id: None }
        );
    }

    #[test]
    fn router_does_not_fabricate_workflow_command_parameters() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 0,
            surface_width: 320.0,
            surface_height: 200.0,
            commands: Vec::new(),
            hit_regions: vec![editor_ui_renderer::HitRegion {
                id: "hit.authoring_workflow_command.scene.secondary".to_string(),
                rect: editor_ui_renderer::UiRect {
                    x: 10.0,
                    y: 10.0,
                    width: 100.0,
                    height: 24.0,
                },
                target: HitTarget::AuthoringWorkflowCommand {
                    command_id: "create_scene_entity".to_string(),
                    payload_kind: "CreateSceneEntity".to_string(),
                    domain: "scene".to_string(),
                },
                enabled: true,
                command_id: Some("create_scene_entity".to_string()),
                reason_disabled: None,
            }],
        };
        let mut router = EditorInputRouter::new();

        let result = router.route(
            EditorInputEvent::PointerDown {
                x: 12.0,
                y: 12.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        assert!(result.command.is_none());
    }

    fn interaction_tree(policy: ActivationPolicy, enabled: bool) -> EditorWidgetTree {
        let mut root = editor_ui_renderer::EditorWidgetDeclaration::new(
            WidgetId::semantic("root").unwrap(),
            WidgetRole::Root,
        );
        let mut button = editor_ui_renderer::EditorWidgetDeclaration::new(
            WidgetId::semantic("button").unwrap(),
            WidgetRole::Button,
        )
        .with_absolute_rect(
            editor_ui_renderer::UiRect {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 30.0,
            },
            1,
        )
        .with_control_style(
            ["decision-control"],
            editor_ui_renderer::ControlPseudoStateSet::empty(),
            policy,
        );
        button.enabled = enabled;
        button.binding = Some(editor_ui_renderer::EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "play".to_string(),
            target: HitTarget::ToolbarCommand {
                command_id: "play".to_string(),
            },
            reason_disabled: (!enabled).then(|| "disabled".to_string()),
        });
        root.children.push(button);
        let (mut tree, _) = editor_ui_renderer::reconcile_widget_tree(None, &root).unwrap();
        let mut measure = |_: &WidgetId, _: Option<f32>| (0.0, 0.0);
        editor_ui_renderer::layout_widget_tree(&mut tree, 200.0, 100.0, &mut measure).unwrap();
        tree
    }

    #[test]
    fn widget_interaction_release_inside_captures_restores_and_activates_once() {
        let tree = interaction_tree(ActivationPolicy::ReleaseInside, true);
        let mut machine = EditorWidgetInteractionMachine::new();
        let down = machine.handle_event(
            &EditorInputEvent::PointerDown {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert!(down.activation.is_none());
        assert_eq!(
            machine
                .snapshot()
                .active_widget_id
                .as_ref()
                .unwrap()
                .as_str(),
            "button"
        );
        machine.handle_event(&EditorInputEvent::PointerMove { x: 150.0, y: 80.0 }, &tree);
        assert!(machine.snapshot().active_widget_id.is_none());
        assert!(machine.snapshot().captured_widget_id.is_some());
        machine.handle_event(&EditorInputEvent::PointerMove { x: 20.0, y: 20.0 }, &tree);
        assert!(machine.snapshot().active_widget_id.is_some());
        let up = machine.handle_event(
            &EditorInputEvent::PointerUp {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert_eq!(up.activation.as_ref().unwrap().as_str(), "button");
        assert!(machine.snapshot().captured_widget_id.is_none());
    }

    #[test]
    fn widget_interaction_release_outside_and_focus_lost_cancel() {
        let tree = interaction_tree(ActivationPolicy::ReleaseInside, true);
        let mut machine = EditorWidgetInteractionMachine::new();
        machine.handle_event(
            &EditorInputEvent::PointerDown {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        let outside = machine.handle_event(
            &EditorInputEvent::PointerUp {
                x: 150.0,
                y: 80.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert!(outside.activation.is_none());
        machine.handle_event(
            &EditorInputEvent::PointerDown {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        machine.handle_event(&EditorInputEvent::FocusLost, &tree);
        assert_eq!(machine.snapshot(), &WidgetInteractionSnapshot::default());
    }

    #[test]
    fn widget_interaction_press_and_keyboard_are_exactly_once() {
        let tree = interaction_tree(ActivationPolicy::Press, true);
        let mut machine = EditorWidgetInteractionMachine::new();
        let down = machine.handle_event(
            &EditorInputEvent::PointerDown {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert!(down.activation.is_some());
        let up = machine.handle_event(
            &EditorInputEvent::PointerUp {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert!(up.activation.is_none());

        machine.set_keyboard_focus(Some(WidgetId::semantic("button").unwrap()));
        let key_down = machine.handle_event(
            &EditorInputEvent::KeyDown {
                key: "Space".to_string(),
            },
            &tree,
        );
        assert!(key_down.activation.is_none());
        assert!(machine.snapshot().focus_visible);
        let key_up = machine.handle_event(
            &EditorInputEvent::KeyUp {
                key: "Space".to_string(),
            },
            &tree,
        );
        assert!(key_up.activation.is_some());
    }

    #[test]
    fn widget_interaction_disabled_never_captures_or_activates() {
        let tree = interaction_tree(ActivationPolicy::ReleaseInside, false);
        let mut machine = EditorWidgetInteractionMachine::new();
        let down = machine.handle_event(
            &EditorInputEvent::PointerDown {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Primary,
            },
            &tree,
        );
        assert_eq!(down.disabled.as_ref().unwrap().as_str(), "button");
        assert!(down.activation.is_none());
        assert!(machine.snapshot().active_widget_id.is_none());
        assert!(machine.snapshot().captured_widget_id.is_none());
    }

    fn fixture_model() -> EditorUiModel {
        EditorUiModel {
            revision: 1,
            frame: 0,
            mode: EditorUiMode::AuthoringWorkspace,
            project_launcher: ProjectLauncherModel::empty(),
            project_intent: editor_ui_model::ProjectIntentWorkspaceModel::empty(),
            project_browser: ProjectBrowserModel {
                project_root: Some("D:/Projects/PlaneGame".to_string()),
                selected_path: Some("Scenes/Main.scene.json".to_string()),
                entries: vec![ProjectBrowserEntry::new(
                    "Scenes/Main.scene.json",
                    "Main.scene.json",
                    ProjectBrowserEntryKind::Scene,
                    true,
                    true,
                    true,
                )],
                empty_message: "No project entries.".to_string(),
            },
            asset_browser: editor_ui_model::AssetBrowserModel::empty(),
            animator2d_authoring: Animator2DAuthoringModel::default(),
            build_export: BuildExportModel {
                selected_profile_id: Some("windows-dev".to_string()),
                profiles: vec![BuildProfileSummary {
                    profile_id: "windows-dev".to_string(),
                    label: "Windows Dev".to_string(),
                    target: "windows".to_string(),
                    output_dir: "D:/Projects/PlaneGame/Build/Windows/dev".to_string(),
                    active: true,
                }],
                release_profile: None,
                commands: vec![
                    BuildExportCommand::new("export_desktop_package", "Export", true, None),
                    BuildExportCommand::new(
                        "build_and_run_desktop_package",
                        "Build & Run",
                        true,
                        None,
                    ),
                    BuildExportCommand::new("build_release_package", "Build Release", true, None),
                    BuildExportCommand::new("begin_asset_pick", "Pick Icon", true, None),
                    BuildExportCommand::new("save_release_profile", "Save Profile", true, None),
                    BuildExportCommand::new("open_build_output", "Output", true, None),
                    BuildExportCommand::new("open_build_report", "Report", true, None),
                ],
                last_report: None,
                last_release_report: None,
                empty_message: String::new(),
            },
            report_panel: editor_ui_model::ReportPanelModel::empty(),
            input_mapping_authoring: editor_ui_model::InputMappingAuthoringModel::empty(),
            rule_authoring: editor_ui_model::RuleAuthoringModel::empty(),
            project_authoring_workspace: editor_ui_model::ProjectAuthoringWorkspaceModel::empty(),
            authoring_workflow: editor_ui_model::AuthoringWorkflowModel::empty(),
            workspace_view_mode: WorkspaceViewMode::SceneView,
            active_runtime_package: None,
            panels: PanelLayoutModel::fixed_mvp(),
            toolbar: ToolbarModel {
                commands: vec![ToolbarCommand {
                    command_id: "tick_one_frame".to_string(),
                    label: "Tick".to_string(),
                    enabled: true,
                    reason_disabled: None,
                }],
                runtime_state: RuntimeRunState::Paused,
                game_view_layout: editor_ui_model::GameViewLayoutState::default(),
            },
            hierarchy: HierarchyModel {
                scene_id: Some("scene-main".to_string()),
                selected_entity_id: None,
                roots: vec![HierarchyNode {
                    entity_id: "entity-player".to_string(),
                    label: "Player".to_string(),
                    alive: true,
                    children: Vec::new(),
                }],
                authoring_view: editor_ui_model::HierarchyAuthoringView::EntityTree,
                visual_order: None,
                source_domain: editor_ui_model::HierarchySourceDomain::AuthoringScene,
                status: "authoring_scene".to_string(),
            },
            inspector: InspectorModel {
                selected_entity_id: Some("entity-player".to_string()),
                title: "Player".to_string(),
                sections: vec![InspectorSection {
                    section_id: "transform".to_string(),
                    title: "Transform".to_string(),
                    fields: vec![InspectorField {
                        field_id: "transform.localPosition".to_string(),
                        label: "localPosition".to_string(),
                        value: InspectorValue::Vec3(Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        }),
                        value_type: InspectorValueType::Vec3,
                        path: "transform.localPosition".to_string(),
                        readonly: false,
                        editable: true,
                    }],
                }],
                readonly: false,
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
            },
            viewport: ViewportModel {
                scene_id: Some("scene-main".to_string()),
                frame: 0,
                frame_hash: None,
                texture_id: None,
                target_id: None,
                renderable_count: 0,
                selected_entity: None,
                renderables: Vec::new(),
                collider_overlay: editor_ui_model::ColliderOverlayModel::default(),
            },
            console: ConsoleModel {
                entries: Vec::new(),
                unread_error_count: 0,
                unread_warning_count: 0,
            },
            runtime_trace: RuntimeTraceModel {
                frame: 0,
                entries: Vec::new(),
                selected_entry_id: None,
            },
            ai_panel: AiPanelModel {
                prompt_placeholder: "Describe an editor change...".to_string(),
                prompt_draft: "create an entity".to_string(),
                messages: vec![AiPanelMessage {
                    message_id: "ai-message-1".to_string(),
                    role: AiPanelMessageRole::Assistant,
                    text: "Ready.".to_string(),
                }],
                gateway_access: Default::default(),
                proposed_commands: vec![AiProposedCommand {
                    proposal_id: "proposal-1".to_string(),
                    label: "Rename selected".to_string(),
                    explanation: "Rename selected entity.".to_string(),
                    command: UiCommandPayload::RenameSceneEntity {
                        entity_id: "entity-player".to_string(),
                        name: "Hero".to_string(),
                    },
                    project_patch: None,
                    imported_project_patch: None,
                    review_state: AiCommandReviewState::Proposed,
                }],
                allowed_command_ids: vec!["rename_scene_entity".to_string()],
                busy: false,
                stage: editor_ui_model::AiPanelStage::Idle,
                status_summary: None,
            },
            project_runtime_trust_prompt: None,
            interaction_feedback: None,
            diagnostics: Vec::new(),
        }
    }
}
