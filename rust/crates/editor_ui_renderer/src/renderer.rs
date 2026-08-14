use editor_ui_model::{
    EditorCommandFeedback, EditorCommandFeedbackStatus, EditorLocaleId, EditorMessageArgs,
    EditorMessageKey, EditorMessageValue, EditorUiMode, EditorUiModel, EDITOR_LOCALE_ZH_CN,
};
use engine_runtime::game_view_presentation::{
    GameViewPresentationModule, GameViewPresentationSpec, GameViewRect,
};
use std::collections::BTreeMap;

use crate::layout::{content_rect, push_border, EditorShellLayout};
use crate::metrics::EditorUiMetrics;
use crate::panels::{
    push_ai_panel, push_build_export_panel, push_console_entries, push_hierarchy,
    push_hierarchy_actions, push_inspector_fields, push_project_browser, push_project_intent_panel,
    push_project_launcher, push_runtime_trace_entries, push_toolbar, push_viewport_header,
    push_workspace_panel_chrome, push_workspace_summary_panel, push_workspace_tabs,
};
use crate::{
    DockNode, DrawCommand, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration,
    EditorWidgetTree, EditorWorkspaceDockingModule, HitTarget, LayoutNodeId, PanelId,
    ReconcileReport, UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetId, WidgetPaint,
    WidgetRole, WorkspaceSnapshot,
};

pub struct SelfUiRenderer;

#[derive(Default)]
pub struct RetainedEditorUiRenderer {
    tree: Option<EditorWidgetTree>,
    last_reconcile: ReconcileReport,
    detached_state: BTreeMap<WidgetId, (WidgetRole, crate::WidgetLocalState)>,
}

impl RetainedEditorUiRenderer {
    pub fn build_draw_list(
        &mut self,
        model: &EditorUiModel,
        config: UiRendererConfig,
    ) -> UiDrawList {
        let localization = config.localization.clone();
        let width = config.width;
        let height = config.height;
        if let Some(tree) = &self.tree {
            self.detached_state.extend(
                tree.nodes
                    .values()
                    .map(|node| (node.id.clone(), (node.role, node.local_state.clone()))),
            );
        }
        let (input, declarations) = SelfUiRenderer::build_widget_scene(model, config);
        let (mut tree, _, report) =
            retained_tree_from_widget_scene(input, declarations, self.tree.as_ref());
        for node in tree.nodes.values_mut() {
            if let Some((role, state)) = self.detached_state.get(&node.id) {
                if *role == node.role {
                    node.local_state = state.clone();
                }
            }
        }
        crate::layout_widget_tree(
            &mut tree,
            width,
            height,
            &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
        )
        .expect("restored editor retained layout");
        let mut draw_list =
            crate::extract_widget_tree(&tree, model.revision, model.frame, width, height).draw_list;
        EditorUiMetrics::apply_typography_scale(&mut draw_list);
        crate::localize_editor_draw_list(&mut draw_list, &localization);
        self.tree = Some(tree);
        self.last_reconcile = report;
        draw_list
    }

    pub fn tree(&self) -> Option<&EditorWidgetTree> {
        self.tree.as_ref()
    }

    pub fn last_reconcile(&self) -> &ReconcileReport {
        &self.last_reconcile
    }

    pub fn scroll_at(&mut self, point: crate::UiPoint, delta: f32) -> Option<crate::WidgetId> {
        let tree = self.tree.as_mut()?;
        let picked = crate::pick_widget(tree, point, None)?;
        let scroll_id = picked
            .path
            .0
            .iter()
            .rev()
            .find(|id| {
                tree.node(id)
                    .is_some_and(|node| node.role == WidgetRole::Scroll)
            })?
            .clone();
        let node = tree.node(&scroll_id)?;
        let current = node.local_state.scroll_y;
        let content_bottom = node
            .children
            .iter()
            .filter_map(|id| tree.node(id))
            .map(|child| child.logical_rect.y + child.logical_rect.height + current)
            .fold(node.logical_rect.y, f32::max);
        let max_scroll =
            (content_bottom - (node.logical_rect.y + node.logical_rect.height)).max(0.0);
        tree.node_mut(&scroll_id)?.local_state.scroll_y = (current + delta).clamp(0.0, max_scroll);
        Some(scroll_id)
    }
}

impl SelfUiRenderer {
    pub fn build_draw_list(model: &EditorUiModel, config: UiRendererConfig) -> UiDrawList {
        let localization = config.localization.clone();
        let (input, declarations) = Self::build_widget_scene(model, config);
        let mut draw_list = retained_tree_from_widget_scene(input, declarations, None).1;
        EditorUiMetrics::apply_typography_scale(&mut draw_list);
        crate::localize_editor_draw_list(&mut draw_list, &localization);
        draw_list
    }

    fn build_widget_scene(
        model: &EditorUiModel,
        config: UiRendererConfig,
    ) -> (UiDrawList, Vec<crate::EditorWidgetDeclaration>) {
        let mut list = UiDrawList {
            revision: model.revision,
            frame: model.frame,
            surface_width: config.width,
            surface_height: config.height,
            commands: Vec::new(),
            hit_regions: Vec::new(),
        };

        if model.mode == EditorUiMode::ProjectLauncher {
            let mut overlays = push_project_launcher(&mut list, model, &config);
            overlays.extend(push_language_menu(
                &mut list,
                UiRect {
                    x: (config.width - 116.0).max(8.0),
                    y: 8.0,
                    width: 108.0,
                    height: 24.0,
                },
                config.language_menu_open,
                &config.localization.locale,
                "launcher",
            ));
            push_interaction_feedback(&mut list, model, &config);
            if let Some(prompt) = &model.project_runtime_trust_prompt {
                let (rect, interactions) = crate::panels::push_project_runtime_trust_prompt(
                    &mut list,
                    prompt,
                    config.width,
                    config.height,
                    &config,
                );
                overlays.push(panel_subtree(
                    "editor/project-runtime-trust",
                    WidgetRole::Panel,
                    rect,
                    interactions,
                ));
            }
            return (
                list,
                vec![panel_subtree(
                    "editor/project-launcher",
                    WidgetRole::Scroll,
                    UiRect {
                        x: 0.0,
                        y: 0.0,
                        width: config.width,
                        height: config.height,
                    },
                    overlays,
                )],
            );
        }

        let shell = EditorShellLayout::resolve(config.width, config.height);
        let mut fallback_workspace =
            EditorWorkspaceDockingModule::standard_editor().snapshot(shell.workspace);
        if config.workspace_snapshot.is_none() {
            if let Some(panel_id) = config.active_bottom_panel_id.as_deref() {
                activate_snapshot_panel(&mut fallback_workspace, panel_id);
            }
        }
        let workspace = config
            .workspace_snapshot
            .as_ref()
            .unwrap_or(&fallback_workspace);
        push_editor_root(&mut list, &config);
        let menu_widgets = push_menu_bar(
            &mut list,
            shell.menu_bar,
            workspace,
            config.workspace_menu_open,
            config.language_menu_open,
            &config.localization.locale,
        );
        let mut overlays = vec![panel_subtree_with_clip(
            "editor/shell/menu",
            WidgetRole::Panel,
            shell.menu_bar,
            menu_widgets,
            false,
        )];
        let toolbar_widgets =
            push_toolbar(&mut list, shell.toolbar, &model.toolbar.commands, &config);
        overlays.push(panel_subtree_with_clip(
            "editor/shell/toolbar",
            WidgetRole::Panel,
            shell.toolbar,
            toolbar_widgets,
            false,
        ));

        let mut stacks = Vec::new();
        collect_workspace_stacks(&workspace.root, &mut stacks);
        for (stack_id, tabs, active_panel_id) in stacks {
            let rect = workspace
                .node_rects
                .get(stack_id)
                .copied()
                .unwrap_or(shell.workspace);
            push_workspace_panel(
                &mut list,
                &mut overlays,
                rect,
                active_panel_id.as_str(),
                model,
                &config.localization,
                &config,
            );
            let tab_labels = tabs
                .iter()
                .map(|panel_id| {
                    (
                        panel_id.as_str().to_string(),
                        workspace_panel_title(panel_id.as_str(), model),
                    )
                })
                .collect::<Vec<_>>();
            let tab_viewport = UiRect {
                width: (rect.width - EditorUiMetrics::PANEL_HEADER_HEIGHT * 2.0).max(1.0),
                ..rect
            };
            let mut tab_widgets = push_workspace_tabs(
                &mut list,
                tab_viewport,
                stack_id.as_str(),
                &tab_labels,
                active_panel_id.as_str(),
                &config,
            );
            let panel = workspace
                .panel_descriptors
                .iter()
                .find(|panel| panel.panel_id == *active_panel_id);
            tab_widgets.extend(push_workspace_panel_chrome(
                &mut list,
                crate::panels::WorkspacePanelChromeSpec {
                    stack_rect: rect,
                    stack_id: stack_id.as_str(),
                    panel_id: active_panel_id.as_str(),
                    lock_available: active_panel_id.as_str() == "inspector"
                        && workspace.inspector_lock_available,
                    locked: active_panel_id.as_str() == "inspector" && workspace.inspector_locked,
                    closable: panel.is_some_and(|panel| panel.closable),
                    popup_open: config.workspace_panel_popup_stack_id.as_deref()
                        == Some(stack_id.as_str()),
                },
                &config,
            ));
            let tab_root_id = if stack_id.as_str() == "workspace/bottom" {
                "editor/dock/bottom-tabs".to_string()
            } else {
                format!("editor/workspace/stack/{}/tabs", stack_id.as_str())
            };
            overlays.push(panel_subtree(
                &tab_root_id,
                WidgetRole::Panel,
                UiRect {
                    height: if config.workspace_panel_popup_stack_id.as_deref()
                        == Some(stack_id.as_str())
                    {
                        EditorUiMetrics::PANEL_HEADER_HEIGHT + EditorUiMetrics::POPUP_ROW_HEIGHT
                    } else {
                        EditorUiMetrics::PANEL_HEADER_HEIGHT
                    },
                    ..rect
                },
                tab_widgets,
            ));
        }

        overlays.extend(workspace_splitter_declarations(workspace));
        if let Some(preview) = &workspace.drag_preview {
            list.commands.push(DrawCommand::Rect {
                rect: preview.rect,
                color: UiColor {
                    a: 72,
                    ..UiColor::ACCENT
                },
                corner_radius: 0.0,
            });
            push_border(&mut list, preview.rect);
        }
        push_interaction_feedback(&mut list, model, &config);
        if let Some(prompt) = &model.project_runtime_trust_prompt {
            let (rect, interactions) = crate::panels::push_project_runtime_trust_prompt(
                &mut list,
                prompt,
                config.width,
                config.height,
                &config,
            );
            overlays.push(panel_subtree(
                "editor/project-runtime-trust",
                WidgetRole::Panel,
                rect,
                interactions,
            ));
        }
        (list, overlays)
    }
}

fn activate_snapshot_panel(snapshot: &mut WorkspaceSnapshot, panel_id: &str) {
    fn activate(node: &mut DockNode, panel_id: &str) -> bool {
        match node {
            DockNode::Split { first, second, .. } => {
                activate(first, panel_id) || activate(second, panel_id)
            }
            DockNode::Stack {
                node_id,
                tabs,
                active_panel_id,
            } => {
                let Some(panel) = tabs.iter().find(|panel| panel.as_str() == panel_id) else {
                    return false;
                };
                *active_panel_id = panel.clone();
                let _ = node_id;
                true
            }
        }
    }
    if activate(&mut snapshot.root, panel_id) {
        snapshot.active_tabs.clear();
        refresh_snapshot_active_tabs(&snapshot.root, &mut snapshot.active_tabs);
    }
}

fn refresh_snapshot_active_tabs(
    node: &DockNode,
    active_tabs: &mut BTreeMap<LayoutNodeId, PanelId>,
) {
    match node {
        DockNode::Split { first, second, .. } => {
            refresh_snapshot_active_tabs(first, active_tabs);
            refresh_snapshot_active_tabs(second, active_tabs);
        }
        DockNode::Stack {
            node_id,
            active_panel_id,
            ..
        } => {
            active_tabs.insert(node_id.clone(), active_panel_id.clone());
        }
    }
}

fn collect_workspace_stacks<'a>(
    node: &'a DockNode,
    output: &mut Vec<(&'a LayoutNodeId, &'a [PanelId], &'a PanelId)>,
) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_workspace_stacks(first, output);
            collect_workspace_stacks(second, output);
        }
        DockNode::Stack {
            node_id,
            tabs,
            active_panel_id,
        } => output.push((node_id, tabs, active_panel_id)),
    }
}

fn workspace_panel_title(panel_id: &str, model: &EditorUiModel) -> String {
    match panel_id {
        "hierarchy" => "Hierarchy".to_string(),
        "viewport" => "Game".to_string(),
        "inspector" => model.inspector.title.clone(),
        "asset_browser" => "Project".to_string(),
        "console" => "Console".to_string(),
        "runtime_trace" => "Trace".to_string(),
        "authoring_workflow" => "Workflow".to_string(),
        "input_mapping" => "Input".to_string(),
        "build_export" => "Build".to_string(),
        "ai_panel" => "AI".to_string(),
        "project_intent" => "Intent".to_string(),
        "report" => "Report".to_string(),
        other => other.replace('_', " "),
    }
}

fn push_workspace_panel(
    list: &mut UiDrawList,
    overlays: &mut Vec<EditorWidgetDeclaration>,
    rect: UiRect,
    panel_id: &str,
    model: &EditorUiModel,
    localization: &editor_ui_model::EditorLocalizationSnapshot,
    config: &UiRendererConfig,
) {
    match panel_id {
        "hierarchy" => {
            push_panel_body(list, rect);
            let mut widgets = push_hierarchy_actions(list, rect, model);
            push_hierarchy(list, rect, &model.hierarchy, 0, &mut widgets);
            overlays.push(panel_subtree(
                "editor/panel/hierarchy",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "viewport" => {
            push_panel_body(list, rect);
            let mut widgets =
                push_viewport_header(list, rect, &model.toolbar.game_view_layout, config);
            let viewport_rect = game_view_content_rect(content_rect(rect), config);
            list.commands.push(DrawCommand::ViewportTextureSlot {
                rect: viewport_rect,
                scene_id: model.viewport.scene_id.clone(),
                frame: model.viewport.frame,
                texture_id: model.viewport.texture_id.clone(),
                target_id: model.viewport.target_id.clone(),
            });
            widgets.push(crate::panels::widget_interaction(
                crate::panels::WidgetInteractionSpec {
                    id: "hit.viewport".to_string(),
                    rect: viewport_rect,
                    role: WidgetRole::Viewport,
                    target: HitTarget::Viewport,
                    enabled: true,
                    command_id: "select_viewport".to_string(),
                    reason_disabled: None,
                },
            ));
            overlays.push(panel_subtree(
                "editor/panel/viewport",
                WidgetRole::Panel,
                rect,
                widgets,
            ));
        }
        "inspector" => {
            push_panel_body(list, rect);
            let widgets = push_inspector_fields(list, rect, model);
            overlays.push(panel_subtree(
                "editor/panel/inspector",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "asset_browser" => {
            let widgets = push_project_browser(list, rect, model, config);
            overlays.push(panel_subtree(
                "editor/panel/asset-browser",
                WidgetRole::Panel,
                rect,
                widgets,
            ));
        }
        "authoring_workflow" | "input_mapping" => {
            let widgets = push_workspace_summary_panel(list, rect, model);
            overlays.push(panel_subtree(
                if panel_id == "input_mapping" {
                    "editor/panel/input-mapping"
                } else {
                    "editor/panel/authoring-workflow"
                },
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "build_export" => {
            let widgets = push_build_export_panel(list, rect, model, localization);
            overlays.push(panel_subtree(
                "editor/panel/build-export",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "ai_panel" => {
            let widgets = push_ai_panel(list, rect, model, config);
            overlays.push(panel_subtree(
                "editor/panel/ai",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "project_intent" => {
            let widgets = push_project_intent_panel(list, rect, model);
            overlays.push(panel_subtree(
                "editor/panel/project-intent",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        "runtime_trace" => {
            let widgets = push_runtime_trace_entries(list, rect, model);
            overlays.push(panel_subtree(
                "editor/panel/runtime-trace",
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
        panel_id => {
            let widgets = push_console_entries(list, rect, model);
            overlays.push(panel_subtree(
                if panel_id == "report" {
                    "editor/panel/report"
                } else {
                    "editor/panel/console"
                },
                WidgetRole::Scroll,
                rect,
                widgets,
            ));
        }
    }
}

fn game_view_content_rect(slot: UiRect, config: &UiRendererConfig) -> UiRect {
    let Some(target) = config.game_view_target else {
        return slot;
    };
    let Ok(presentation) = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "editor-game-view-display".to_string(),
        target_id: "editor-game-view".to_string(),
        target_extent: target.extent,
        display_rect: GameViewRect::new(slot.x, slot.y, slot.width, slot.height),
        scale_policy: target.scale_policy,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: Vec::new(),
    }) else {
        return slot;
    };
    UiRect {
        x: presentation.display_content_rect.x,
        y: presentation.display_content_rect.y,
        width: presentation.display_content_rect.width,
        height: presentation.display_content_rect.height,
    }
}

fn workspace_splitter_declarations(snapshot: &WorkspaceSnapshot) -> Vec<EditorWidgetDeclaration> {
    snapshot
        .splitters
        .iter()
        .flat_map(|splitter| {
            let node_id = splitter.node_id.as_str();
            let mut hit_declaration = EditorWidgetDeclaration::new(
                WidgetId::semantic(format!("editor/workspace/splitter/{node_id}"))
                    .expect("workspace splitter WidgetId"),
                WidgetRole::Splitter,
            )
            .with_absolute_rect(splitter.hit_rect, 100_000_010);
            hit_declaration.hit_region_id = Some(format!("hit.workspace_splitter.{node_id}"));
            hit_declaration.binding = Some(EditorCommandBinding {
                action: EditorWidgetAction::Resize,
                command_id: "resize_workspace_splitter".to_string(),
                target: HitTarget::WorkspaceSplitter {
                    node_id: node_id.to_string(),
                },
                reason_disabled: None,
            });
            let mut visual_declaration = EditorWidgetDeclaration::new(
                WidgetId::semantic(format!("editor/workspace/splitter-visual/{node_id}"))
                    .expect("workspace splitter visual WidgetId"),
                WidgetRole::Overlay,
            )
            .with_absolute_rect(splitter.visual_rect, 100_000_000);
            visual_declaration.paint.push(WidgetPaint::Rect {
                color: UiColor::BORDER,
                corner_radius: 0.0,
            });
            [hit_declaration, visual_declaration]
        })
        .collect()
}

fn retained_tree_from_widget_scene(
    input: UiDrawList,
    declarations: Vec<EditorWidgetDeclaration>,
    previous: Option<&EditorWidgetTree>,
) -> (EditorWidgetTree, UiDrawList, ReconcileReport) {
    let mut root = EditorWidgetDeclaration::new(
        WidgetId::semantic("editor/root").expect("static editor root id"),
        WidgetRole::Root,
    );
    for (index, command) in input.commands.iter().enumerate() {
        root.children.push(draw_declaration(index, command));
    }
    debug_assert!(
        input.hit_regions.is_empty(),
        "production widgets must author interactions as declarations"
    );
    root.children.extend(declarations);
    let (mut tree, report) =
        crate::reconcile_widget_tree(previous, &root).expect("editor widget declaration invariant");
    crate::layout_widget_tree(
        &mut tree,
        input.surface_width,
        input.surface_height,
        &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
    )
    .expect("editor retained layout");
    let draw_list = crate::extract_widget_tree(
        &tree,
        input.revision,
        input.frame,
        input.surface_width,
        input.surface_height,
    )
    .draw_list;
    (tree, draw_list, report)
}

fn draw_declaration(index: usize, command: &DrawCommand) -> EditorWidgetDeclaration {
    let command = command.unclipped();
    let rect = match command {
        DrawCommand::Rect { rect, .. }
        | DrawCommand::Text { rect, .. }
        | DrawCommand::ViewportTextureSlot { rect, .. }
        | DrawCommand::ImageTextureSlot { rect, .. } => *rect,
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    };
    let role = match command {
        DrawCommand::Text { .. } => WidgetRole::Label,
        DrawCommand::ImageTextureSlot { .. } => WidgetRole::Image,
        DrawCommand::ViewportTextureSlot { .. } => WidgetRole::Viewport,
        DrawCommand::Rect { .. } => WidgetRole::Container,
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    };
    let paint = match command {
        DrawCommand::Rect {
            color,
            corner_radius,
            ..
        } => crate::WidgetPaint::Rect {
            color: *color,
            corner_radius: *corner_radius,
        },
        DrawCommand::Text {
            text, color, size, ..
        } => crate::WidgetPaint::Text {
            text: text.clone(),
            color: *color,
            size: *size,
        },
        DrawCommand::ImageTextureSlot {
            texture_id,
            fallback_color,
            source_uv,
            tint,
            ..
        } => crate::WidgetPaint::Image {
            texture_id: texture_id.clone(),
            fallback_color: *fallback_color,
            source_uv: *source_uv,
            tint: *tint,
        },
        DrawCommand::ViewportTextureSlot {
            scene_id,
            frame,
            texture_id,
            target_id,
            ..
        } => crate::WidgetPaint::Viewport {
            scene_id: scene_id.clone(),
            frame: *frame,
            texture_id: texture_id.clone(),
            target_id: target_id.clone(),
        },
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    };
    let mut declaration = EditorWidgetDeclaration::new(
        WidgetId::semantic(format!("editor/paint/{index}")).expect("generated editor paint id"),
        role,
    )
    .with_absolute_rect(rect, index as i32);
    declaration.paint.push(paint);
    declaration
}

fn panel_subtree(
    id: &str,
    role: WidgetRole,
    panel: UiRect,
    children: Vec<EditorWidgetDeclaration>,
) -> EditorWidgetDeclaration {
    panel_subtree_with_clip(id, role, panel, children, true)
}

fn panel_subtree_with_clip(
    id: &str,
    role: WidgetRole,
    panel: UiRect,
    mut children: Vec<EditorWidgetDeclaration>,
    clip: bool,
) -> EditorWidgetDeclaration {
    for child in &mut children {
        child.style.inset_left = child.style.inset_left.map(|x| x - panel.x);
        child.style.inset_top = child.style.inset_top.map(|y| y - panel.y);
    }
    let mut root =
        EditorWidgetDeclaration::new(WidgetId::semantic(id).expect("static panel root id"), role)
            .with_absolute_rect(panel, 60_000);
    root.style.clip = clip;
    root.children = children;
    root
}

fn push_editor_root(list: &mut UiDrawList, config: &UiRendererConfig) {
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: 0.0,
            y: 0.0,
            width: config.width,
            height: config.height,
        },
        color: UiColor::ROOT,
        corner_radius: 0.0,
    });
}

fn push_menu_bar(
    list: &mut UiDrawList,
    rect: UiRect,
    workspace: &WorkspaceSnapshot,
    menu_open: bool,
    language_menu_open: bool,
    active_locale: &EditorLocaleId,
) -> Vec<EditorWidgetDeclaration> {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::MENU,
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 6.0,
            y: rect.y + 4.0,
            width: rect.width - 12.0,
            height: 14.0,
        },
        text: "AI First Engine".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    let button = UiRect {
        x: rect.x + 140.0,
        y: rect.y + 2.0,
        width: 80.0,
        height: (rect.height - 4.0).max(1.0),
    };
    list.commands.push(DrawCommand::Rect {
        rect: button,
        color: if menu_open {
            UiColor::TAB_ACTIVE
        } else {
            UiColor::MENU
        },
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: button.x + 8.0,
            y: button.y + 2.0,
            width: button.width - 16.0,
            height: 14.0,
        },
        text: "Window".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    let mut window_button = EditorWidgetDeclaration::new(
        WidgetId::semantic("editor/shell/menu/window").expect("window menu id"),
        WidgetRole::Button,
    )
    .with_absolute_rect(button, 110_000_000);
    window_button.hit_region_id = Some("hit.workspace.window_menu".to_string());
    window_button.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: "toggle_workspace_window_menu".to_string(),
        target: HitTarget::WorkspaceWindowMenu,
        reason_disabled: None,
    });
    let mut declarations = vec![window_button];
    declarations.extend(push_language_menu(
        list,
        UiRect {
            x: button.x + button.width + 4.0,
            y: button.y,
            width: 92.0,
            height: button.height,
        },
        language_menu_open,
        active_locale,
        "workspace",
    ));
    if !menu_open {
        return declarations;
    }

    let registry = crate::PanelRegistry::standard_editor();
    let closable = registry
        .panel_ids()
        .filter_map(|panel_id| registry.get(panel_id.as_str()))
        .filter(|descriptor| descriptor.closable)
        .collect::<Vec<_>>();
    let popup = UiRect {
        x: button.x,
        y: rect.y + rect.height,
        width: 220.0,
        height: (closable.len() as f32 + 1.0) * EditorUiMetrics::COMPACT_CONTROL_HEIGHT + 8.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: popup,
        color: crate::EditorTheme::DARK_NEUTRAL.surface.popup,
        corner_radius: 0.0,
    });
    push_border(list, popup);
    for (index, descriptor) in closable.into_iter().enumerate() {
        let visible = workspace
            .panel_rects
            .contains_key(descriptor.panel_id.as_str());
        let item = UiRect {
            x: popup.x + 4.0,
            y: popup.y + 4.0 + index as f32 * EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
            width: popup.width - 8.0,
            height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 2.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: item.x + 8.0,
                y: item.y + 4.0,
                width: item.width - 16.0,
                height: 14.0,
            },
            text: format!(
                "{} {}",
                if visible { "[x]" } else { "[ ]" },
                descriptor.title
            ),
            color: UiColor::TEXT,
            size: 11.0,
        });
        let panel_id = descriptor.panel_id.as_str();
        let mut declaration = EditorWidgetDeclaration::new(
            WidgetId::semantic(format!("editor/shell/menu/window/panel/{panel_id}"))
                .expect("window panel item id"),
            WidgetRole::Button,
        )
        .with_absolute_rect(item, 120_000_000 + index as i32);
        declaration.hit_region_id = Some(format!("hit.workspace.panel_visibility.{panel_id}"));
        declaration.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "set_workspace_panel_visibility".to_string(),
            target: HitTarget::WorkspacePanelVisibility {
                panel_id: panel_id.to_string(),
                visible,
            },
            reason_disabled: None,
        });
        declarations.push(declaration);
    }
    let reset = UiRect {
        x: popup.x + 4.0,
        y: popup.y
            + 4.0
            + (declarations.len() - 1) as f32 * EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
        width: popup.width - 8.0,
        height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 2.0,
    };
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: reset.x + 8.0,
            y: reset.y + 4.0,
            width: reset.width - 16.0,
            height: 14.0,
        },
        text: "Reset Layout".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    let mut reset_declaration = EditorWidgetDeclaration::new(
        WidgetId::semantic("editor/shell/menu/window/reset").expect("window reset id"),
        WidgetRole::Button,
    )
    .with_absolute_rect(reset, 120_100_000);
    reset_declaration.hit_region_id = Some("hit.workspace.reset_layout".to_string());
    reset_declaration.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: "reset_workspace_layout".to_string(),
        target: HitTarget::WorkspaceResetLayout,
        reason_disabled: None,
    });
    declarations.push(reset_declaration);
    declarations
}

fn push_language_menu(
    list: &mut UiDrawList,
    button: UiRect,
    menu_open: bool,
    active_locale: &EditorLocaleId,
    scope: &str,
) -> Vec<EditorWidgetDeclaration> {
    list.commands.push(DrawCommand::Rect {
        rect: button,
        color: if menu_open {
            UiColor::TAB_ACTIVE
        } else {
            UiColor::MENU
        },
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: button.x + 8.0,
            y: button.y + 4.0,
            width: button.width - 16.0,
            height: 16.0,
        },
        text: "Language".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });

    let mut button_declaration = EditorWidgetDeclaration::new(
        WidgetId::semantic(format!("editor/{scope}/language")).expect("language menu widget id"),
        WidgetRole::Button,
    )
    .with_absolute_rect(button, 130_000_000);
    button_declaration.hit_region_id = Some("hit.editor.language_menu".to_string());
    button_declaration.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: "toggle_editor_language_menu".to_string(),
        target: HitTarget::EditorLanguageMenu,
        reason_disabled: None,
    });
    let mut declarations = vec![button_declaration];
    if !menu_open {
        return declarations;
    }

    let choices = [
        (EditorLocaleId::zh_cn(), "简体中文（zh-CN）"),
        (EditorLocaleId::en_us(), "English (en-US)"),
    ];
    let popup = UiRect {
        x: button.x,
        y: button.y + button.height,
        width: 220.0,
        height: choices.len() as f32 * EditorUiMetrics::COMPACT_CONTROL_HEIGHT + 8.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: popup,
        color: crate::EditorTheme::DARK_NEUTRAL.surface.popup,
        corner_radius: 0.0,
    });
    push_border(list, popup);
    for (index, (locale, self_name)) in choices.into_iter().enumerate() {
        let item = UiRect {
            x: popup.x + 4.0,
            y: popup.y + 4.0 + index as f32 * EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
            width: popup.width - 8.0,
            height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 2.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: item.x + 8.0,
                y: item.y + 4.0,
                width: item.width - 16.0,
                height: 16.0,
            },
            text: format!(
                "{} {self_name}",
                if active_locale == &locale {
                    "[x]"
                } else {
                    "[ ]"
                }
            ),
            color: UiColor::TEXT,
            size: 11.0,
        });
        let mut choice = EditorWidgetDeclaration::new(
            WidgetId::semantic(format!("editor/{scope}/language/{}", locale.as_str()))
                .expect("locale choice widget id"),
            WidgetRole::Button,
        )
        .with_absolute_rect(item, 140_000_000 + index as i32);
        choice.hit_region_id = Some(format!("hit.editor.locale.{}", locale.as_str()));
        choice.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "set_editor_locale".to_string(),
            target: HitTarget::SetEditorLocale { locale },
            reason_disabled: None,
        });
        declarations.push(choice);
    }
    declarations
}

fn push_interaction_feedback(
    list: &mut UiDrawList,
    model: &EditorUiModel,
    config: &UiRendererConfig,
) {
    let Some(feedback) = &model.interaction_feedback else {
        return;
    };
    if !matches!(
        feedback.status,
        EditorCommandFeedbackStatus::Rejected | EditorCommandFeedbackStatus::Failed
    ) {
        return;
    }
    let banner = UiRect {
        x: 8.0,
        y: (config.height - 48.0).max(0.0),
        width: (config.width - 16.0).max(0.0),
        height: 40.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: banner,
        color: crate::EditorTheme::DARK_NEUTRAL.status.error_surface,
        corner_radius: 4.0,
    });
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            width: 4.0_f32.min(banner.width),
            ..banner
        },
        color: UiColor::ERROR,
        corner_radius: 2.0,
    });
    let text_rect = UiRect {
        x: banner.x + 12.0,
        y: banner.y + 12.0,
        width: (banner.width - 24.0).max(0.0),
        height: 16.0,
    };
    let raw_message = feedback
        .reason
        .as_deref()
        .unwrap_or(feedback.message.as_str());
    let message = localized_interaction_feedback(feedback, raw_message, &config.localization);
    list.commands.push(DrawCommand::Text {
        rect: text_rect,
        text: truncate_feedback_for_width(&message, text_rect.width),
        color: UiColor::TEXT,
        size: 12.0,
    });
}

fn localized_interaction_feedback(
    feedback: &EditorCommandFeedback,
    raw_message: &str,
    localization: &editor_ui_model::EditorLocalizationSnapshot,
) -> String {
    if let Some(localized) = localization.localize_native_exact(raw_message) {
        return localized;
    }
    if localization.locale.as_str() != EDITOR_LOCALE_ZH_CN {
        return raw_message.to_string();
    }
    let key = EditorMessageKey::parse("editor.feedback.operation_failed")
        .expect("trusted feedback message key");
    let code = feedback
        .diagnostic_code
        .as_deref()
        .unwrap_or(feedback.command_id.as_str());
    let args = EditorMessageArgs::from([(
        "code".to_string(),
        EditorMessageValue::StableId(code.to_string()),
    )]);
    localization
        .resolve(&key, &args)
        .unwrap_or_else(|_| raw_message.to_string())
}

fn truncate_feedback_for_width(message: &str, width: f32) -> String {
    let max_chars = (width / 7.0).floor().max(1.0) as usize;
    let count = message.chars().count();
    if count <= max_chars {
        return message.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut value = message.chars().take(max_chars - 3).collect::<String>();
    value.push_str("...");
    value
}

fn push_panel_body(list: &mut UiDrawList, rect: UiRect) {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::PANEL,
        corner_radius: 0.0,
    });
    push_border(list, rect);
}
