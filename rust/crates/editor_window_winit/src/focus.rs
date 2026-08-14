use editor_input::EditorInputEvent;
use editor_ui_renderer::{
    EditorWidgetTree, HitTarget, UiDrawList, UiPoint, WidgetId, WidgetRole, WidgetVisibility,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorMainFrame {
    pub title: String,
    pub layout_version: String,
    pub root_layout_name: String,
}

impl Default for EditorMainFrame {
    fn default() -> Self {
        Self {
            title: "AI First Engine Editor".to_string(),
            layout_version: "native-editor-main-frame.v1".to_string(),
            root_layout_name: "DefaultEditorLayout".to_string(),
        }
    }
}

#[derive(Default)]
pub struct EditorFocusInputSystem {
    pub hovered_panel_id: Option<String>,
    pub active_panel_id: Option<String>,
    pub hovered_hit_id: Option<String>,
    pub pressed_hit_id: Option<String>,
    pub keyboard_focus: Option<WidgetId>,
    pub pointer_capture: Option<WidgetId>,
    pub mouse_captured: bool,
    pub last_pointer_position: Option<(f32, f32)>,
    pub consumed_text_input_count: u64,
}

impl EditorFocusInputSystem {
    pub fn observe_event(
        &mut self,
        event: &EditorInputEvent,
        draw_list: &UiDrawList,
        tree: Option<&EditorWidgetTree>,
    ) {
        match event {
            EditorInputEvent::PointerDown { x, y, .. } => {
                self.last_pointer_position = Some((*x, *y));
                self.mouse_captured = true;
                let hit = editor_ui_renderer::hit_test_any(draw_list, UiPoint { x: *x, y: *y });
                self.active_panel_id =
                    hit.and_then(|region| panel_id_for_hit_target(&region.target));
                if let Some(region) = hit {
                    let widget_id = tree.and_then(|tree| widget_id_for_hit_id(tree, &region.id));
                    let styled_control = widget_id
                        .as_ref()
                        .and_then(|id| tree.and_then(|tree| tree.node(id)))
                        .is_some_and(|node| !node.control_classes.as_slice().is_empty());
                    self.pointer_capture = (!styled_control).then_some(widget_id.clone()).flatten();
                    self.pressed_hit_id = (!styled_control).then(|| region.id.clone());
                    if region.enabled {
                        self.keyboard_focus = widget_id;
                    }
                } else {
                    self.pressed_hit_id = None;
                }
            }
            EditorInputEvent::PointerUp { x, y, .. } => {
                self.last_pointer_position = Some((*x, *y));
                self.mouse_captured = false;
                self.pointer_capture = None;
                self.pressed_hit_id = None;
            }
            EditorInputEvent::PointerMove { x, y } => {
                self.last_pointer_position = Some((*x, *y));
                let hit = editor_ui_renderer::hit_test_any(draw_list, UiPoint { x: *x, y: *y });
                self.hovered_hit_id = hit.and_then(|region| {
                    let styled_control = tree
                        .and_then(|tree| widget_id_for_hit_id(tree, &region.id))
                        .as_ref()
                        .and_then(|id| tree.and_then(|tree| tree.node(id)))
                        .is_some_and(|node| !node.control_classes.as_slice().is_empty());
                    (!styled_control).then(|| region.id.clone())
                });
                self.hovered_panel_id =
                    hit.and_then(|region| panel_id_for_hit_target(&region.target));
            }
            EditorInputEvent::FocusLost => {
                self.mouse_captured = false;
                self.pointer_capture = None;
                self.pressed_hit_id = None;
                self.keyboard_focus = None;
            }
            EditorInputEvent::MouseWheel { .. }
            | EditorInputEvent::KeyDown { .. }
            | EditorInputEvent::KeyUp { .. } => {}
        }
    }

    pub fn focus_widget(&mut self, widget_id: WidgetId) {
        self.keyboard_focus = Some(widget_id);
    }

    pub fn consume_text_input(&mut self, _text: &str, tree: Option<&EditorWidgetTree>) -> bool {
        if self
            .keyboard_focus
            .as_ref()
            .and_then(|id| tree.and_then(|tree| tree.node(id)))
            .is_some_and(|node| node.role == WidgetRole::TextInput)
        {
            self.consumed_text_input_count += 1;
            return true;
        }
        false
    }

    pub fn focused_target<'a>(&self, tree: Option<&'a EditorWidgetTree>) -> Option<&'a HitTarget> {
        let node = tree?.node(self.keyboard_focus.as_ref()?)?;
        node.binding.as_ref().map(|binding| &binding.target)
    }

    pub fn focus_next(&mut self, tree: &EditorWidgetTree, reverse: bool) -> Option<WidgetId> {
        let popup_open = tree
            .nodes
            .values()
            .any(|node| node.hit_region_id.as_deref() == Some("hit.toolbar.overflow.barrier"));
        let mut candidates = tree
            .nodes
            .values()
            .filter(|node| {
                node.visibility == WidgetVisibility::Visible
                    && node.enabled
                    && node.role.can_activate()
                    && (!popup_open
                        || node
                            .id
                            .as_str()
                            .starts_with("editor/shell/toolbar/overflow/"))
                    && node.hit_region_id.as_deref() != Some("hit.toolbar.overflow.barrier")
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|node| node.resolved_z);
        if candidates.is_empty() {
            self.keyboard_focus = None;
            return None;
        }
        let current = self
            .keyboard_focus
            .as_ref()
            .and_then(|id| candidates.iter().position(|node| &node.id == id));
        let next = if reverse {
            current
                .map(|index| index.checked_sub(1).unwrap_or(candidates.len() - 1))
                .unwrap_or(candidates.len() - 1)
        } else {
            current.map_or(0, |index| (index + 1) % candidates.len())
        };
        let id = candidates[next].id.clone();
        self.keyboard_focus = Some(id.clone());
        Some(id)
    }
}

fn widget_id_for_hit_id(tree: &EditorWidgetTree, hit_id: &str) -> Option<WidgetId> {
    tree.nodes
        .values()
        .find(|node| node.hit_region_id.as_deref() == Some(hit_id))
        .map(|node| node.id.clone())
}

fn panel_id_for_hit_target(target: &HitTarget) -> Option<String> {
    match target {
        HitTarget::ProjectLauncherAction { .. }
        | HitTarget::ProjectLauncherRecentProject { .. }
        | HitTarget::ProjectRuntimeTrustDecision { .. } => Some("project_launcher".to_string()),
        HitTarget::ProjectBrowserEntry { .. }
        | HitTarget::ProjectBrowserOpen { .. }
        | HitTarget::AssetBrowserEntry { .. }
        | HitTarget::AssetBrowserOpen { .. }
        | HitTarget::AssetBrowserFolder { .. }
        | HitTarget::AssetBrowserAction { .. }
        | HitTarget::AssetPickerConfirm
        | HitTarget::AssetPickerCancel
        | HitTarget::AssetBrowserSearch => Some("asset_browser".to_string()),
        HitTarget::AuthoringWorkflowStep { .. } | HitTarget::AuthoringWorkflowCommand { .. } => {
            Some("authoring_workflow".to_string())
        }
        HitTarget::ToolbarCommand { .. } => Some("toolbar".to_string()),
        HitTarget::HierarchyEntity { .. } | HitTarget::HierarchyAction { .. } => {
            Some("hierarchy".to_string())
        }
        HitTarget::InspectorField { .. } | HitTarget::InspectorAssetPicker { .. } => {
            Some("inspector".to_string())
        }
        HitTarget::RuntimeTraceEntry { .. } => Some("runtime_trace".to_string()),
        HitTarget::AiProposedCommand { .. }
        | HitTarget::GatewayAccessDecision { .. }
        | HitTarget::GatewayAccessPage { .. }
        | HitTarget::AiPanelAction { .. }
        | HitTarget::AiPromptField => Some("ai_panel".to_string()),
        HitTarget::ProjectIntentAction { .. } => Some("project_intent".to_string()),
        HitTarget::InputMappingControl { .. } => Some("input_mapping".to_string()),
        HitTarget::DockTab { panel_id } => Some(panel_id.clone()),
        HitTarget::WorkspacePanelLock { panel_id, .. }
        | HitTarget::WorkspacePanelMore { panel_id, .. }
        | HitTarget::WorkspacePanelClose { panel_id, .. } => Some(panel_id.clone()),
        HitTarget::WorkspaceSplitter { .. } => None,
        HitTarget::WorkspaceWindowMenu
        | HitTarget::EditorLanguageMenu
        | HitTarget::SetEditorLocale { .. }
        | HitTarget::WorkspacePanelVisibility { .. }
        | HitTarget::WorkspaceResetLayout => Some("menu".to_string()),
        HitTarget::ToolbarOverflow => Some("toolbar".to_string()),
        HitTarget::ConsoleEntry { .. } => Some("console".to_string()),
        HitTarget::Viewport | HitTarget::GameViewTarget { .. } | HitTarget::AuiSceneNode { .. } => {
            Some("viewport".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_renderer::{
        extract_widget_tree, layout_widget_tree, reconcile_widget_tree, EditorCommandBinding,
        EditorWidgetAction, EditorWidgetDeclaration,
    };

    fn focus_fixture() -> (EditorWidgetTree, UiDrawList) {
        let mut root = EditorWidgetDeclaration::new(
            WidgetId::semantic("focus/root").unwrap(),
            WidgetRole::Root,
        );
        for (index, role) in [WidgetRole::Button, WidgetRole::TextInput]
            .into_iter()
            .enumerate()
        {
            let mut node = EditorWidgetDeclaration::new(
                WidgetId::semantic(format!("focus/{index}")).unwrap(),
                role,
            )
            .with_absolute_rect(
                editor_ui_renderer::UiRect {
                    x: index as f32 * 40.0,
                    y: 0.0,
                    width: 30.0,
                    height: 20.0,
                },
                index as i32,
            );
            node.hit_region_id = Some(format!("hit.focus.{index}"));
            node.binding = Some(EditorCommandBinding {
                action: EditorWidgetAction::Activate,
                command_id: format!("focus_{index}"),
                target: if role == WidgetRole::TextInput {
                    HitTarget::AssetBrowserSearch
                } else {
                    HitTarget::ToolbarCommand {
                        command_id: format!("focus_{index}"),
                    }
                },
                reason_disabled: None,
            });
            root.children.push(node);
        }
        let (mut tree, _) = reconcile_widget_tree(None, &root).unwrap();
        layout_widget_tree(
            &mut tree,
            100.0,
            40.0,
            &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
        )
        .unwrap();
        let draw = extract_widget_tree(&tree, 1, 1, 100.0, 40.0).draw_list;
        (tree, draw)
    }

    #[test]
    fn tab_focus_uses_widget_ids_and_wraps() {
        let (tree, _) = focus_fixture();
        let mut focus = EditorFocusInputSystem::default();
        assert_eq!(focus.focus_next(&tree, false).unwrap().as_str(), "focus/0");
        assert_eq!(focus.focus_next(&tree, false).unwrap().as_str(), "focus/1");
        assert_eq!(focus.focus_next(&tree, false).unwrap().as_str(), "focus/0");
        assert_eq!(focus.focus_next(&tree, true).unwrap().as_str(), "focus/1");
    }

    #[test]
    fn focus_lost_clears_widget_capture_pressed_and_keyboard_focus() {
        let (tree, draw) = focus_fixture();
        let mut focus = EditorFocusInputSystem::default();
        focus.observe_event(
            &EditorInputEvent::PointerDown {
                x: 1.0,
                y: 1.0,
                button: editor_input::PointerButton::Primary,
            },
            &draw,
            Some(&tree),
        );
        assert!(focus.pointer_capture.is_some());
        assert!(focus.keyboard_focus.is_some());
        focus.observe_event(&EditorInputEvent::FocusLost, &draw, Some(&tree));
        assert!(focus.pointer_capture.is_none());
        assert!(focus.pressed_hit_id.is_none());
        assert!(focus.keyboard_focus.is_none());
    }
}
