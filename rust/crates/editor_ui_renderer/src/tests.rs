use super::*;
use editor_ui_model::{
    AiCommandReviewState, AiPanelMessage, AiPanelMessageRole, AiPanelModel, AiProposedCommand,
    Animator2DAuthoringModel, BuildExportCommand, BuildExportModel, BuildExportReportSummary,
    BuildProfileSummary, ConsoleModel, EditorCommandFeedback, EditorCommandFeedbackStatus,
    EditorUiMode, EditorUiModel, GatewayAccessInboxModel, GatewayAccessRequestModel,
    HierarchyModel, InspectorField, InspectorModel, InspectorSection, InspectorValue,
    InspectorValueType, PanelLayoutModel, ProjectBrowserEntry, ProjectBrowserEntryKind,
    ProjectBrowserModel, ProjectLauncherModel, ProjectOpenActivityModel, ProjectOpenActivityPhase,
    RecentProjectEntry, ReleaseBuildProfileModel, RuntimeRunState, RuntimeTraceModel,
    ToolbarCommand, ToolbarModel, UiCommandPayload, UiCommandSource, Vec3, ViewportModel,
    WorkspaceViewMode,
};
use engine_runtime::game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec};

#[test]
fn editor_typography_scale_raises_all_production_text_and_control_rows() {
    let model = fixture_model();
    let workspace = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_workspace_panel_chrome(Some("workspace/right".to_string())),
    );
    let mut launcher_model = model.clone();
    launcher_model.mode = EditorUiMode::ProjectLauncher;
    let launcher =
        SelfUiRenderer::build_draw_list(&launcher_model, UiRendererConfig::new(1280.0, 720.0));

    for (surface, draw) in [("workspace", &workspace), ("launcher", &launcher)] {
        let text_sizes = draw
            .commands
            .iter()
            .filter_map(|command| match command.unclipped() {
                DrawCommand::Text { size, .. } => Some(*size),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!text_sizes.is_empty(), "{surface} must paint text");
        assert!(
            text_sizes.iter().all(|size| *size >= 13.0),
            "{surface} still contains a pre-scale font size: {text_sizes:?}"
        );
    }

    let tab = workspace
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::DockTab { .. }))
        .expect("workspace tab");
    assert!(
        tab.rect.height >= 29.0,
        "Tab paint/hit height must follow the +5 typography scale"
    );
    let hierarchy_row = workspace
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::HierarchyEntity { .. }))
        .expect("Hierarchy row");
    assert!(
        hierarchy_row.rect.height >= 29.0,
        "Hierarchy row must follow the +5 typography scale"
    );
    let popup = workspace
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::WorkspacePanelClose { .. }))
        .expect("Panel popup row");
    assert!(
        popup.rect.height >= 33.0,
        "Panel popup row must preserve vertical padding around 16px text"
    );
}

#[test]
fn workspace_panel_chrome_reserves_tabs_and_builds_retained_close_popup() {
    let model = fixture_model();
    let snapshot = EditorWorkspaceDockingModule::standard_editor().snapshot(UiRect {
        x: 0.0,
        y: 52.0,
        width: 1280.0,
        height: 668.0,
    });
    let base = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_workspace_panel_chrome(None)
            .with_workspace_snapshot({
                let mut snapshot = snapshot.clone();
                snapshot.inspector_lock_available = true;
                snapshot
            }),
    );
    let right_more = base
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspacePanelMore { stack_id, panel_id }
                    if stack_id == "workspace/right" && panel_id == "inspector"
            )
        })
        .expect("Inspector More");
    let right_lock = base
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspacePanelLock { stack_id, panel_id, .. }
                    if stack_id == "workspace/right" && panel_id == "inspector"
            )
        })
        .expect("Inspector Lock");
    assert_eq!(
        right_more.rect.width,
        crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT
    );
    assert_eq!(
        right_lock.rect.width,
        crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT
    );
    assert_eq!(right_lock.rect.x + right_lock.rect.width, right_more.rect.x);
    assert!(base
        .hit_regions
        .iter()
        .filter(|region| matches!(
            &region.target,
            HitTarget::DockTab { panel_id } if panel_id == "inspector"
        ))
        .all(|tab| tab.rect.x + tab.rect.width <= right_lock.rect.x));
    let unsupported_lock = base
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspacePanelLock { panel_id, .. } if panel_id == "viewport"
            )
        })
        .expect("unsupported panel Lock");
    assert!(!unsupported_lock.enabled);
    assert!(unsupported_lock.reason_disabled.is_some());

    let popup = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_workspace_panel_chrome(Some("workspace/right".to_string()))
            .with_workspace_snapshot({
                let mut snapshot = snapshot;
                snapshot.inspector_lock_available = true;
                snapshot.inspector_locked = true;
                snapshot
            }),
    );
    let popup_items = popup
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::WorkspacePanelClose { .. }))
        .collect::<Vec<_>>();
    assert_eq!(popup_items.len(), 1);
    assert!(!popup_items[0].enabled);
    assert_eq!(
        popup_items[0].reason_disabled.as_deref(),
        Some("This panel cannot be closed.")
    );
}

#[test]
fn panel_header_visual_remediation_owns_one_title_and_uses_compact_icons() {
    let model = fixture_model();
    let mut snapshot = EditorWorkspaceDockingModule::standard_editor().snapshot(UiRect {
        x: 0.0,
        y: 52.0,
        width: 1280.0,
        height: 668.0,
    });
    snapshot.inspector_lock_available = true;
    let draw = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_workspace_panel_chrome(None)
            .with_workspace_snapshot(snapshot),
    );
    let lock = draw
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspacePanelLock { panel_id, .. } if panel_id == "inspector"
            )
        })
        .expect("Inspector Lock");
    let more = draw
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspacePanelMore { panel_id, .. } if panel_id == "inspector"
            )
        })
        .expect("Inspector More");
    let inspector_header_title_count = draw
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                DrawCommand::Text { rect, text, .. }
                    if text == &model.inspector.title
                        && rect.y >= lock.rect.y
                        && rect.y < lock.rect.y + lock.rect.height
            )
        })
        .count();
    assert_eq!(
        inspector_header_title_count, 1,
        "the Stack header must be the sole Inspector title owner"
    );
    let is_within = |rect: &UiRect, bounds: UiRect| {
        rect.x >= bounds.x
            && rect.y >= bounds.y
            && rect.x + rect.width <= bounds.x + bounds.width
            && rect.y + rect.height <= bounds.y + bounds.height
    };
    assert!(
        !draw.commands.iter().any(|command| {
            matches!(
                command,
                DrawCommand::Text { rect, text, .. }
                    if (is_within(rect, lock.rect) || is_within(rect, more.rect))
                        && matches!(text.as_str(), "l" | "L" | "...")
            )
        }),
        "Panel chrome must not use placeholder text glyphs"
    );
    let compact_rect_count = |bounds: UiRect| {
        draw.commands
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    DrawCommand::Rect { rect, .. }
                        if is_within(rect, bounds)
                            && rect.width <= 12.0
                            && rect.height <= 12.0
                )
            })
            .count()
    };
    assert!(
        compact_rect_count(lock.rect) >= 3,
        "Lock must use compact renderer-native geometry"
    );
    assert_eq!(
        compact_rect_count(more.rect),
        3,
        "More must be exactly three compact vertical dots"
    );
}

#[test]
fn header_paint_containment_keeps_tab_paint_out_of_panel_body() {
    let model = fixture_model();
    let snapshot = EditorWorkspaceDockingModule::standard_editor().snapshot(UiRect {
        x: 0.0,
        y: 52.0,
        width: 1280.0,
        height: 668.0,
    });
    let left_stack = snapshot
        .node_rects
        .get("workspace/left")
        .copied()
        .expect("left Stack rect");
    let draw = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0).with_workspace_snapshot(snapshot),
    );
    let selected_row = draw
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::HierarchyEntity { entity_id } if entity_id == "entity-player"
            )
        })
        .expect("selected Hierarchy row")
        .rect;
    let selected_paint_index = draw
        .commands
        .iter()
        .position(|command| {
            matches!(
                command.unclipped(),
                DrawCommand::Rect { rect, color, .. }
                    if *rect == selected_row && *color == UiColor::ACCENT
            )
        })
        .expect("selected Hierarchy accent paint");
    let occluding_panel_paints = draw
        .commands
        .iter()
        .skip(selected_paint_index + 1)
        .filter(|command| {
            matches!(
                command.unclipped(),
                DrawCommand::Rect { rect, color, .. }
                    if *color == UiColor::PANEL && rect.intersection(selected_row).is_some()
            )
        })
        .count();
    assert_eq!(
        occluding_panel_paints, 0,
        "Tab paint must not cover the selected Hierarchy row"
    );
    let chrome_boundary_x = left_stack.x + left_stack.width - 48.0 - 1.0;
    assert!(
        !draw.commands.iter().any(|command| {
            matches!(
                command.unclipped(),
                DrawCommand::Rect { rect, color, .. }
                    if *color == UiColor::BORDER
                        && (rect.x - chrome_boundary_x).abs() < f32::EPSILON
                        && rect.height > 24.0
            )
        }),
        "the Tab viewport border must not extend below the 24px header"
    );
}

#[test]
fn renderer_outputs_draw_commands_and_hit_regions_from_model() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("console"));
    let ai_draw = SelfUiRenderer::build_draw_list(&model, config_with_panel("ai_panel"));
    let asset_draw = SelfUiRenderer::build_draw_list(&model, config_with_panel("asset_browser"));
    let build_draw = SelfUiRenderer::build_draw_list(&model, config_with_panel("build_export"));
    let workflow_draw =
        SelfUiRenderer::build_draw_list(&model, config_with_panel("authoring_workflow"));
    assert!(draw_list.commands.len() > 10);
    assert!(draw_list
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::ViewportTextureSlot { scene_id: Some(id), texture_id: Some(texture_id), target_id: Some(target_id), .. } if id == "scene-main" && texture_id == "viewport-scene::frame-3" && target_id == "viewport-scene")));
    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.toolbar.tick_one_frame"));
    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.hierarchy.entity-player"));
    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.inspector_field.transform.localPosition"));
    assert!(ai_draw
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.ai_proposal.accept.proposal-1"));
    assert!(asset_draw
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AssetBrowserEntry { .. })));
    assert!(asset_draw
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AssetBrowserOpen { .. })));
    assert!(build_draw
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.build_export.export_desktop_package"));
    assert!(build_draw
        .hit_regions
        .iter()
        .any(|region| region.command_id.as_deref() == Some("open_build_report")));
    assert!(workflow_draw
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.authoring_workflow_step.build"
            && region.command_id.as_deref() == Some("set_authoring_workflow_step")));
    assert!(workflow_draw.hit_regions.iter().any(|region| {
        region.id == "hit.authoring_workflow_command.build.primary"
            && matches!(
                &region.target,
                HitTarget::AuthoringWorkflowCommand {
                    command_id,
                    payload_kind,
                    domain,
                } if command_id == "export_desktop_package"
                    && payload_kind == "ExportDesktopPackage"
                    && domain == "build"
            )
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text == "AI First Engine")
    }));
    assert!(build_draw.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("显示器 1"))
    }));
    assert!(draw_list
        .commands
        .iter()
        .any(|command| { matches!(command, DrawCommand::Text { text, .. } if text == "项目") }));
    assert!(build_draw.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text == "构建与导出")
    }));
}

#[test]
fn game_view_contain_uses_one_rect_for_texture_and_input() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(
        &model,
        config_with_panel("console").with_game_view_target(Some(GameViewTargetSpec::new(
            720,
            1280,
            GameViewScalePolicy::Contain,
        ))),
    );
    let texture_rect = draw_list
        .commands
        .iter()
        .find_map(|command| match command.unclipped() {
            DrawCommand::ViewportTextureSlot { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("GameView texture slot");
    let input_rect = draw_list
        .hit_regions
        .iter()
        .find(|region| region.target == HitTarget::Viewport)
        .expect("GameView input region")
        .rect;

    assert_eq!(texture_rect, input_rect);
    assert!(
        texture_rect.height > texture_rect.width,
        "portrait Contain content must remain portrait: {texture_rect:?}"
    );

    let stretched = SelfUiRenderer::build_draw_list(
        &model,
        config_with_panel("console").with_game_view_target(Some(GameViewTargetSpec::new(
            720,
            1280,
            GameViewScalePolicy::Stretch,
        ))),
    );
    let stretched_texture_rect = stretched
        .commands
        .iter()
        .find_map(|command| match command.unclipped() {
            DrawCommand::ViewportTextureSlot { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("stretched GameView texture slot");
    let stretched_input_rect = stretched
        .hit_regions
        .iter()
        .find(|region| region.target == HitTarget::Viewport)
        .expect("stretched GameView input region")
        .rect;

    assert_eq!(stretched_texture_rect, stretched_input_rect);
    assert!(stretched_texture_rect.width > stretched_texture_rect.height);
    assert!(texture_rect.width < stretched_texture_rect.width);
    assert_eq!(texture_rect.height, stretched_texture_rect.height);
}

#[test]
fn production_panels_have_zero_manual_hit_authoring() {
    assert_no_manual_hit_authoring();
}

#[test]
fn game_view_target_selector_is_typed_visible_non_overlapping_and_play_guarded() {
    let mut model = fixture_model();
    model.toolbar.game_view_layout.target = editor_ui_model::EditorGameViewTarget::new(
        720,
        1280,
        editor_ui_model::EditorGameViewScalePolicy::Contain,
    );
    model.toolbar.game_view_layout.target_editable = true;
    let draw = SelfUiRenderer::build_draw_list(&model, config_with_panel("viewport"));
    let mut targets = draw
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::GameViewTarget { .. }))
        .collect::<Vec<_>>();
    assert!(
        targets.len() >= 3,
        "expected target presets in GameView header"
    );
    assert!(targets.iter().all(|region| region.enabled));
    assert!(targets.iter().any(|region| {
        matches!(
            region.target,
            HitTarget::GameViewTarget {
                width: 720,
                height: 1280,
                scale_policy: editor_ui_model::EditorGameViewScalePolicy::Contain
            }
        )
    }));
    targets.sort_by(|left, right| left.rect.x.total_cmp(&right.rect.x));
    for pair in targets.windows(2) {
        assert!(
            pair[0].rect.x + pair[0].rect.width <= pair[1].rect.x,
            "GameView target controls overlap: {:?} then {:?}",
            pair[0].rect,
            pair[1].rect
        );
    }

    model.toolbar.game_view_layout.target_editable = false;
    let guarded = SelfUiRenderer::build_draw_list(&model, config_with_panel("viewport"));
    assert!(guarded
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::GameViewTarget { .. }))
        .all(|region| !region.enabled));
}

#[test]
fn dock_tab_widgets_switch_visible_panel_without_overlaying_inactive_content() {
    let model = fixture_model();
    let console = SelfUiRenderer::build_draw_list(&model, config_with_panel("console"));
    let ai = SelfUiRenderer::build_draw_list(&model, config_with_panel("ai_panel"));

    assert_eq!(
        console
            .hit_regions
            .iter()
            .filter(|region| matches!(region.target, HitTarget::DockTab { .. }))
            .count(),
        native_editor_panel_manifest()
            .iter()
            .filter(|panel| panel.dockable)
            .count()
    );
    assert!(console.hit_regions.iter().any(|region| {
        matches!(
            &region.target,
            HitTarget::DockTab { panel_id } if panel_id == "project_intent"
        )
    }));
    assert!(!console
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AiPromptField)));
    assert!(ai
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AiPromptField)));
}

#[test]
fn dock_tab_layout_remains_reachable_in_narrow_surface() {
    let model = fixture_model();
    let draw = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(320.0, 480.0).with_active_bottom_panel(Some("console".to_string())),
    );
    let tabs: Vec<_> = draw
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::DockTab { .. }))
        .collect();
    assert_eq!(
        tabs.len(),
        native_editor_panel_manifest()
            .iter()
            .filter(|panel| panel.dockable)
            .count()
    );
    assert!(tabs.iter().any(|region| {
        matches!(
            &region.target,
            HitTarget::DockTab { panel_id } if panel_id == "project_intent"
        )
    }));
    assert!(tabs
        .iter()
        .all(|region| region.rect.x + region.rect.width <= 320.0));
}

#[test]
fn toolbar_overflow_exposes_hidden_commands_in_narrow_surface() {
    let mut model = fixture_model();
    for index in 0..8 {
        model.toolbar.commands.push(ToolbarCommand {
            command_id: format!("extra_{index}"),
            label: format!("Extra {index}"),
            enabled: true,
            reason_disabled: None,
        });
    }
    let closed = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(320.0, 480.0).with_active_bottom_panel(Some("console".to_string())),
    );
    assert!(closed
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::ToolbarOverflow)));

    let open = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(320.0, 480.0)
            .with_active_bottom_panel(Some("console".to_string()))
            .with_toolbar_overflow_open(true),
    );
    assert!(open.hit_regions.iter().any(|region| {
        region.id.starts_with("hit.toolbar.overflow.")
            && matches!(region.target, HitTarget::ToolbarCommand { .. })
    }));
}

#[test]
fn hierarchy_scroll_is_node_local_cached_and_clips_derived_hits() {
    let mut model = fixture_model();
    model.hierarchy.roots = (0..30)
        .map(|index| editor_ui_model::HierarchyNode {
            entity_id: format!("entity-{index}"),
            label: format!("Entity {index}"),
            alive: true,
            children: Vec::new(),
        })
        .collect();
    let mut renderer = RetainedEditorUiRenderer::default();
    let before = renderer.build_draw_list(&model, config_with_panel("console"));
    let first = before
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.hierarchy.entity-0")
        .expect("first hierarchy row")
        .rect;
    let scroll_id = renderer
        .scroll_at(
            UiPoint {
                x: first.x + 2.0,
                y: first.y + 2.0,
            },
            120.0,
        )
        .expect("hierarchy scroll target");
    assert_eq!(scroll_id.as_str(), "editor/panel/hierarchy");

    let after = renderer.build_draw_list(&model, config_with_panel("console"));
    assert!(renderer.last_reconcile().reused > 0);
    assert!(renderer
        .tree()
        .and_then(|tree| tree.node(&scroll_id))
        .is_some_and(|node| node.local_state.scroll_y > 0.0));
    let hierarchy_rect = renderer
        .tree()
        .and_then(|tree| tree.node(&scroll_id))
        .expect("hierarchy retained panel")
        .logical_rect;
    assert!(after
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::HierarchyEntity { .. }))
        .all(|region| {
            region.rect.y >= hierarchy_rect.y
                && region.rect.y + region.rect.height <= hierarchy_rect.y + hierarchy_rect.height
        }));
}

#[test]
fn workspace_splitter_visual_is_hairline_while_hit_region_stays_wide() {
    let model = fixture_model();
    let mut renderer = RetainedEditorUiRenderer::default();
    let draw_list = renderer.build_draw_list(&model, config_with_panel("console"));
    let hit = draw_list
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspaceSplitter { node_id } if node_id == "workspace/top"
            )
        })
        .expect("horizontal workspace splitter hit");
    let tree = renderer.tree().expect("retained tree");
    let hit_node = tree
        .node(
            &WidgetId::semantic("editor/workspace/splitter/workspace/top")
                .expect("splitter hit widget id"),
        )
        .expect("splitter hit widget");
    let visual = tree
        .node(
            &WidgetId::semantic("editor/workspace/splitter-visual/workspace/top")
                .expect("splitter visual widget id"),
        )
        .expect("splitter visual widget");

    assert_eq!(hit.rect.width, 7.0);
    assert_eq!(hit_node.logical_rect, hit.rect);
    assert!(
        hit_node.paint.is_empty(),
        "transparent hit band must not paint"
    );
    assert_eq!(visual.logical_rect.width, 1.0);
    assert!(
        visual.resolved_z < hit_node.resolved_z,
        "visual hairline must paint below the transparent pick target"
    );
    assert_eq!(
        hit.rect.x + hit.rect.width * 0.5,
        visual.logical_rect.x + visual.logical_rect.width * 0.5
    );
    assert!(draw_list.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Rect { rect, color, .. }
                if *rect == visual.logical_rect && *color == UiColor::BORDER
        )
    }));
}

fn assert_no_manual_hit_authoring() {
    for (path, source) in [
        ("src/renderer.rs", include_str!("renderer.rs")),
        ("src/panels/ai_panel.rs", include_str!("panels/ai_panel.rs")),
        (
            "src/panels/build_export.rs",
            include_str!("panels/build_export.rs"),
        ),
        ("src/panels/console.rs", include_str!("panels/console.rs")),
        (
            "src/panels/hierarchy.rs",
            include_str!("panels/hierarchy.rs"),
        ),
        (
            "src/panels/inspector.rs",
            include_str!("panels/inspector.rs"),
        ),
        ("src/panels/launcher.rs", include_str!("panels/launcher.rs")),
        (
            "src/panels/project_browser.rs",
            include_str!("panels/project_browser.rs"),
        ),
        (
            "src/panels/runtime_trace.rs",
            include_str!("panels/runtime_trace.rs"),
        ),
        ("src/panels/toolbar.rs", include_str!("panels/toolbar.rs")),
        (
            "src/panels/workspace.rs",
            include_str!("panels/workspace.rs"),
        ),
    ] {
        assert_eq!(source.matches("hit_regions.push(").count(), 0, "{path}");
        assert_eq!(source.matches("command_hit_region(").count(), 0, "{path}");
        assert_eq!(source.matches("hit_region(").count(), 0, "{path}");
    }
}

#[test]
fn input_mapping_workspace_renders_stable_product_hit_targets() {
    let mut model = fixture_model();
    model.authoring_workflow.active_step = editor_ui_model::AuthoringStepId::Input;
    let mut input = editor_ui_model::InputMappingAuthoringModel::empty();
    input.project_root = Some("D:/Projects/PlaneGame".to_string());
    input.selected_path = Some("Input/input.default.json".to_string());
    input.mapping_id = Some("input.default".to_string());
    input.selected_context_id = Some("gameplay".to_string());
    input.selected_action_id = Some("action.fire".to_string());
    input.selected_binding_id = Some("binding.fire".to_string());
    input.source_hash = Some("fnv1a64:test".to_string());
    input.dirty = true;
    input.contexts = vec![editor_ui_model::InputMappingContextSummary {
        context_id: "gameplay".to_string(),
        priority: 0,
        consume_input: false,
        enabled_by_default: true,
    }];
    input.actions = vec![editor_ui_model::InputMappingActionSummary {
        action_id: "action.fire".to_string(),
        value_type: editor_ui_model::InputActionValueKind::Button,
        binding_count: 1,
    }];
    input.bindings = vec![editor_ui_model::InputMappingBindingSummary {
        binding_id: "binding.fire".to_string(),
        binding_index: 0,
        context_id: "gameplay".to_string(),
        action_id: "action.fire".to_string(),
        device_path: "keyboard/Space".to_string(),
        processor: "None".to_string(),
        trigger: "Pressed".to_string(),
    }];
    input.control_catalog.controls = vec![editor_ui_model::InputControlCatalogEntryModel {
        device_path: "mouse/Left".to_string(),
        label: "Mouse left button".to_string(),
        device_kind: editor_ui_model::InputControlDeviceKindModel::Mouse,
        compatible_value_types: vec![editor_ui_model::InputActionValueKind::Button],
        selectable: true,
        capture_supported: true,
    }];
    model.input_mapping_authoring = input;

    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("input_mapping"));

    for id in [
        "hit.input.command.save.asset",
        "hit.input.context.gameplay",
        "hit.input.action.action.fire",
        "hit.input.binding.binding.fire",
        "hit.input.device.binding.fire.mouse_Left",
        "hit.input.command.begin_capture.binding.fire",
    ] {
        assert!(
            draw_list.hit_regions.iter().any(|region| region.id == id),
            "missing input mapping hit target {id}"
        );
    }
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Input Mapping"))
    }));
}

#[test]
fn aui_scene_hit_test_prefers_late_registered_aui_node_over_viewport() {
    let draw_list = UiDrawList {
        revision: 1,
        frame: 0,
        surface_width: 320.0,
        surface_height: 200.0,
        commands: Vec::new(),
        hit_regions: vec![
            HitRegion {
                id: "hit.viewport".to_string(),
                rect: UiRect {
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
            HitRegion {
                id: "hit.aui.score_text".to_string(),
                rect: UiRect {
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

    let hit = hit_test(&draw_list, UiPoint { x: 32.0, y: 24.0 }).expect("AUI node hit");

    assert_eq!(hit.id, "hit.aui.score_text");
    assert!(matches!(
        &hit.target,
        HitTarget::AuiSceneNode { node_id, .. } if node_id == "score_text"
    ));
}

#[test]
fn renderer_hit_test_resolves_toolbar_command() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("asset_browser"));
    let region = draw_list
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.tick_one_frame")
        .expect("toolbar hit region should exist");
    let hit = hit_test(
        &draw_list,
        UiPoint {
            x: region.rect.x + 2.0,
            y: region.rect.y + 2.0,
        },
    )
    .expect("point should hit toolbar");
    assert_eq!(
        hit.target,
        HitTarget::ToolbarCommand {
            command_id: "tick_one_frame".to_string()
        }
    );
}

#[test]
fn renderer_marks_toolbar_interaction_and_disabled_metadata() {
    let mut model = fixture_model();
    model.toolbar.commands[0].enabled = false;
    model.toolbar.commands[0].reason_disabled = Some("Open a Runtime Package first.".to_string());
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let disabled_region = draw_list
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.tick_one_frame")
        .expect("disabled toolbar region");

    assert!(!disabled_region.enabled);
    assert_eq!(
        disabled_region.reason_disabled.as_deref(),
        Some("Open a Runtime Package first.")
    );
    assert!(hit_test(
        &draw_list,
        UiPoint {
            x: disabled_region.rect.x + 1.0,
            y: disabled_region.rect.y + 1.0
        }
    )
    .is_none());
    assert!(hit_test_any(
        &draw_list,
        UiPoint {
            x: disabled_region.rect.x + 1.0,
            y: disabled_region.rect.y + 1.0
        }
    )
    .is_some());

    model.toolbar.commands[0].enabled = true;
    model.toolbar.commands[0].reason_disabled = None;
    let hovered = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_interaction(Some("hit.toolbar.tick_one_frame".to_string()), None),
    );
    assert!(hovered.commands.iter().any(|command| {
        matches!(command, DrawCommand::Rect { color, .. } if *color == UiColor::PANEL_LIGHT)
    }));

    let pressed = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0).with_interaction(
            Some("hit.toolbar.tick_one_frame".to_string()),
            Some("hit.toolbar.tick_one_frame".to_string()),
        ),
    );
    assert!(pressed.commands.iter().any(|command| {
        matches!(command, DrawCommand::Rect { color, .. } if *color == UiColor::ACCENT)
    }));
}

#[test]
fn renderer_outputs_project_launcher_without_workspace_panels() {
    let mut model = fixture_model();
    model.mode = EditorUiMode::ProjectLauncher;
    model.project_launcher = ProjectLauncherModel::empty();
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));

    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.project_launcher.open_project"));
    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.project_launcher.create_project"));
    assert!(!draw_list
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.toolbar.tick_one_frame"));
    assert!(draw_list
        .commands
        .iter()
        .any(|command| { matches!(command, DrawCommand::Text { text, .. } if text == "项目") }));
}

#[test]
fn launcher_project_open_activity_is_localized_opaque_bounded_and_disables_open_actions() {
    for (width, height) in [(1280.0, 720.0), (1600.0, 900.0)] {
        let mut model = fixture_model();
        model.mode = EditorUiMode::ProjectLauncher;
        model.project_launcher = ProjectLauncherModel::empty();
        model
            .project_launcher
            .recent_projects
            .push(RecentProjectEntry {
                name: "Tower Defense".to_string(),
                path: "G:/gameEngin/samples/tower_defense_project".to_string(),
                engine_version: "0.0.3".to_string(),
                last_opened_at: None,
                last_modified_at: None,
                valid: true,
                status: "ready".to_string(),
            });
        model.project_launcher.activity = Some(ProjectOpenActivityModel {
            operation_id: "open-1".to_string(),
            project_display_name: "Tower Defense".to_string(),
            phase: ProjectOpenActivityPhase::ComputingDigest,
            completed_units: None,
            total_units: None,
            elapsed_ms: 2_400,
            cancellable: false,
            diagnostic_code: None,
            next_action: None,
        });

        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(width, height));
        assert!(draw_list.commands.iter().any(|command| {
            matches!(command, DrawCommand::Text { text, .. } if text == "正在计算项目摘要")
        }));
        assert!(draw_list.commands.iter().any(|command| {
            matches!(command, DrawCommand::Text { text, .. } if text == "已耗时 2 秒")
        }));
        for region in draw_list.hit_regions.iter().filter(|region| {
            region.id == "hit.project_launcher.open_project"
                || region.id == "hit.project_launcher.open_project.top"
                || region.id.starts_with("hit.project_launcher.recent.")
        }) {
            assert!(!region.enabled, "{} remained enabled", region.id);
        }
        for command in &draw_list.commands {
            let rect = match command {
                DrawCommand::Rect { rect, .. } | DrawCommand::Text { rect, .. } => rect,
                _ => continue,
            };
            assert!(rect.x >= 0.0 && rect.y >= 0.0);
            assert!(rect.x + rect.width <= width + 0.1);
            assert!(rect.y + rect.height <= height + 0.1);
        }
    }
}

#[test]
fn project_open_activity_composition_phases_are_localized() {
    for (phase, expected) in [
        (
            ProjectOpenActivityPhase::Inspecting,
            "正在检查项目编辑器输入",
        ),
        (
            ProjectOpenActivityPhase::CacheLookup,
            "正在查找精确项目编辑器",
        ),
        (
            ProjectOpenActivityPhase::Promoting,
            "正在提升已验证的项目编辑器",
        ),
        (ProjectOpenActivityPhase::Warming, "正在预热项目编辑器缓存"),
        (ProjectOpenActivityPhase::Sealing, "正在封装项目编辑器"),
        (ProjectOpenActivityPhase::Cancelled, "项目编辑器准备已取消"),
    ] {
        let mut model = fixture_model();
        model.mode = EditorUiMode::ProjectLauncher;
        model.project_launcher = ProjectLauncherModel::empty();
        model.project_launcher.activity = Some(ProjectOpenActivityModel {
            operation_id: "composition-phase".to_string(),
            project_display_name: "Tower Defense".to_string(),
            phase,
            completed_units: None,
            total_units: None,
            elapsed_ms: 100,
            cancellable: true,
            diagnostic_code: None,
            next_action: None,
        });
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
        assert!(draw_list.commands.iter().any(|command| {
            matches!(command, DrawCommand::Text { text, .. } if text == expected)
        }));
    }
}

#[test]
fn renderer_draws_interaction_feedback_in_launcher_and_workspace() {
    let diagnostic = "project.dialog.windows_set_initial_folder_failed: 0x80070005";
    let mut model = fixture_model();
    model.interaction_feedback = Some(EditorCommandFeedback {
        command_id: "open_project".to_string(),
        status: EditorCommandFeedbackStatus::Rejected,
        diagnostic_code: Some("project.dialog.windows_set_initial_folder_failed".to_string()),
        message: diagnostic.to_string(),
        reason: Some(diagnostic.to_string()),
        source: UiCommandSource::ProjectLauncher,
    });

    for mode in [
        EditorUiMode::ProjectLauncher,
        EditorUiMode::AuthoringWorkspace,
    ] {
        model.mode = mode;
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));
        assert!(draw_list.commands.iter().any(|command| {
            matches!(command, DrawCommand::Text { text, .. } if text == "操作失败（project.dialog.windows_set_initial_folder_failed）。请在控制台查看详细信息。")
        }));
    }

    model.interaction_feedback.as_mut().unwrap().status = EditorCommandFeedbackStatus::Committed;
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));
    assert!(!draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("project.dialog.windows_set_initial_folder_failed"))
    }));
}

#[test]
fn feedback_banner_localizes_cataloged_project_runtime_error() {
    let english = "The running Editor does not contain the requested ProjectRust module.";
    let chinese = "当前运行的编辑器未包含项目所需的 ProjectRust 模块。";
    let mut model = fixture_model();
    model.interaction_feedback = Some(EditorCommandFeedback {
        command_id: "open_project".to_string(),
        status: EditorCommandFeedbackStatus::Rejected,
        diagnostic_code: Some("project.runtime_module_missing".to_string()),
        message: english.to_string(),
        reason: Some(english.to_string()),
        source: UiCommandSource::ProjectLauncher,
    });

    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let texts = draw_list
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        });
    let texts = texts.collect::<Vec<_>>();
    assert!(texts.contains(&chinese), "draw texts: {texts:?}");
    assert!(!texts.contains(&english), "draw texts: {texts:?}");
}

#[test]
fn renderer_outputs_recent_project_hit_region() {
    let mut model = fixture_model();
    model.mode = EditorUiMode::ProjectLauncher;
    model.project_launcher = ProjectLauncherModel::empty();
    model
        .project_launcher
        .recent_projects
        .push(RecentProjectEntry {
            name: "PlaneGame".to_string(),
            path: "D:/Projects/PlaneGame".to_string(),
            engine_version: "0.0.3".to_string(),
            last_opened_at: None,
            last_modified_at: Some("today".to_string()),
            valid: true,
            status: "ready".to_string(),
        });

    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1024.0, 600.0));

    assert!(draw_list
        .commands
        .iter()
        .any(|command| { matches!(command, DrawCommand::Text { text, .. } if text == "today") }));

    assert!(draw_list.hit_regions.iter().any(|region| {
            matches!(&region.target, HitTarget::ProjectLauncherRecentProject { project_path } if project_path == "D:/Projects/PlaneGame")
        }));
}

#[test]
fn launcher_formats_recent_project_epoch_as_unpadded_date() {
    let mut model = fixture_model();
    model.mode = EditorUiMode::ProjectLauncher;
    model.project_launcher = ProjectLauncherModel::empty();
    model
        .project_launcher
        .recent_projects
        .push(RecentProjectEntry {
            name: "Tower Defense".to_string(),
            path: "G:/gameEngin/samples/tower_defense_project".to_string(),
            engine_version: "0.0.3".to_string(),
            last_opened_at: None,
            last_modified_at: Some("223776000".to_string()),
            valid: true,
            status: "ready".to_string(),
        });

    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let texts = draw_list
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        });
    let texts = texts.collect::<Vec<_>>();
    assert!(texts.contains(&"1977.2.3"), "draw texts: {texts:?}");
    assert!(!texts.contains(&"223776000"), "draw texts: {texts:?}");
}

#[test]
fn renderer_hit_test_prefers_asset_browser_open_over_row_select() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("asset_browser"));
    let region = draw_list
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AssetBrowserOpen { .. }))
        .expect("asset browser open hit region");
    let hit = hit_test(
        &draw_list,
        UiPoint {
            x: region.rect.x + 1.0,
            y: region.rect.y + 1.0,
        },
    )
    .expect("open button should hit");

    assert!(matches!(
        &hit.target,
        HitTarget::AssetBrowserOpen { path, .. } if path == "Scenes/Main.scene.json"
    ));
}

#[test]
fn renderer_asset_browser_scroll_window_is_not_limited_to_five_entries() {
    let mut model = fixture_model();
    model.asset_browser.entries = (0..20)
        .map(|index| {
            editor_ui_model::AssetBrowserEntry::authoring(
                format!("Assets/texture-{index}.asset"),
                format!("texture-{index}.asset"),
                editor_ui_model::AssetKind::Texture,
                editor_ui_model::EditorAssetRef::new(format!("texture-{index}"), "texture"),
            )
        })
        .collect();
    let mut renderer = RetainedEditorUiRenderer::default();
    let first_draw = renderer.build_draw_list(&model, config_with_panel("asset_browser"));
    let first_paths = first_draw
        .hit_regions
        .iter()
        .filter_map(|region| match &region.target {
            HitTarget::AssetBrowserEntry { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let first_entry = first_draw
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AssetBrowserEntry { .. }))
        .expect("visible asset entry");
    renderer
        .scroll_at(
            UiPoint {
                x: first_entry.rect.x + 1.0,
                y: first_entry.rect.y + 1.0,
            },
            10.0 * 22.0,
        )
        .expect("asset body scroll widget");
    let scrolled_draw = renderer.build_draw_list(&model, config_with_panel("asset_browser"));
    let scrolled_paths = scrolled_draw
        .hit_regions
        .iter()
        .filter_map(|region| match &region.target {
            HitTarget::AssetBrowserEntry { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(!first_paths.is_empty());
    assert!(scrolled_paths
        .iter()
        .any(|path| { path == "Assets/texture-10.asset" || path == "Assets/texture-11.asset" }));
    assert_ne!(first_paths, scrolled_paths);
    assert!(scrolled_draw.hit_regions.iter().all(|region| {
        !region.id.starts_with("hit.asset_browser.entry.")
            || region.id.len() > "hit.asset_browser.entry.0".len()
    }));
}

#[test]
fn retained_panel_local_scroll_survives_tab_detach_and_restore() {
    let mut model = fixture_model();
    model.asset_browser.entries = (0..20)
        .map(|index| {
            editor_ui_model::AssetBrowserEntry::authoring(
                format!("Assets/texture-{index}.asset"),
                format!("texture-{index}.asset"),
                editor_ui_model::AssetKind::Texture,
                editor_ui_model::EditorAssetRef::new(format!("texture-{index}"), "texture"),
            )
        })
        .collect();
    let mut renderer = RetainedEditorUiRenderer::default();
    let draw = renderer.build_draw_list(&model, config_with_panel("asset_browser"));
    let entry = draw
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AssetBrowserEntry { .. }))
        .expect("visible asset entry");
    let scroll_id = renderer
        .scroll_at(
            UiPoint {
                x: entry.rect.x + 1.0,
                y: entry.rect.y + 1.0,
            },
            120.0,
        )
        .expect("asset scroll widget");
    renderer.build_draw_list(&model, config_with_panel("console"));
    renderer.build_draw_list(&model, config_with_panel("asset_browser"));

    assert!(renderer
        .tree()
        .and_then(|tree| tree.node(&scroll_id))
        .is_some_and(|node| node.local_state.scroll_y > 0.0));
}

#[test]
fn production_tree_exposes_manifest_roots_for_active_surfaces() {
    let model = fixture_model();
    let mut renderer = RetainedEditorUiRenderer::default();
    for (panel_id, root_id) in [
        ("asset_browser", "editor/panel/asset-browser"),
        ("authoring_workflow", "editor/panel/authoring-workflow"),
        ("input_mapping", "editor/panel/input-mapping"),
        ("build_export", "editor/panel/build-export"),
        ("ai_panel", "editor/panel/ai"),
        ("runtime_trace", "editor/panel/runtime-trace"),
        ("console", "editor/panel/console"),
        ("report", "editor/panel/report"),
    ] {
        renderer.build_draw_list(&model, config_with_panel(panel_id));
        let tree = renderer.tree().expect("retained tree");
        for shell_root in [
            "editor/shell/menu",
            "editor/shell/toolbar",
            "editor/panel/hierarchy",
            "editor/panel/viewport",
            "editor/panel/inspector",
            "editor/dock/bottom-tabs",
            root_id,
        ] {
            assert!(
                tree.node(&WidgetId::semantic(shell_root).unwrap())
                    .is_some(),
                "missing manifest root {shell_root} for {panel_id}"
            );
        }
    }
}

#[test]
fn renderer_asset_browser_emits_image_texture_slots_for_visible_and_selected_thumbnail() {
    let mut model = fixture_model();
    let mut texture = editor_ui_model::AssetBrowserEntry::new(
        "Assets/Images/player.png",
        "player.png",
        editor_ui_model::AssetKind::Texture,
    );
    texture.preview.thumbnail_id = Some("asset-thumbnail::player".to_string());
    texture.preview.thumbnail_aspect_ratio =
        editor_ui_model::AssetThumbnailAspectRatio::new(64, 32);
    texture.preview.status = editor_ui_model::AssetPreviewStatus::Ready;
    model.asset_browser.view_mode = editor_ui_model::AssetBrowserViewMode::Grid;
    model.asset_browser.selection = editor_ui_model::AssetSelection::single_entry(&texture);
    texture.selected = true;
    model.asset_browser.entries = vec![texture];

    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("asset_browser"));
    let image_slots = draw_list
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command.unclipped(),
                DrawCommand::ImageTextureSlot {
                    texture_id: Some(texture_id),
                    ..
                } if texture_id == "asset-thumbnail::player"
            )
        })
        .count();

    assert!(
        image_slots >= 2,
        "grid tile and selected preview should both present the image"
    );
}

#[test]
fn renderer_outputs_build_export_panel_and_commands() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("build_export"));

    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text == "构建与导出")
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Windows Dev"))
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Release: Complex Shooter 1.0.0"))
    }));
    for command_id in [
        "export_desktop_package",
        "build_release_package",
        "begin_asset_pick",
        "save_release_profile",
        "open_build_output",
        "open_build_report",
    ] {
        assert!(draw_list.hit_regions.iter().any(|region| {
            region.id == format!("hit.build_export.{command_id}")
                && region.command_id.as_deref() == Some(command_id)
        }));
    }
}

#[test]
fn renderer_outputs_project_authoring_workspace_summary() {
    let model = fixture_model();
    let draw_list =
        SelfUiRenderer::build_draw_list(&model, config_with_panel("authoring_workflow"));

    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text == "工作区")
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Project project-plane"))
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Scene Ready"))
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("workflow"))
    }));
    assert!(draw_list.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.contains("Build Ready"))
    }));
}

fn fixture_model() -> EditorUiModel {
    EditorUiModel {
        revision: 1,
        frame: 3,
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
        asset_browser: fixture_asset_browser_model(),
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
            release_profile: Some(ReleaseBuildProfileModel {
                profile_id: "windows-release".to_string(),
                display_name: "Complex Shooter".to_string(),
                executable_name: "ComplexShooter".to_string(),
                company_name: "AI First Engine Studio".to_string(),
                file_description: "Complex Shooter".to_string(),
                display_version: "1.0.0".to_string(),
                architecture: "x86_64".to_string(),
                icon_asset_id: "app-icon".to_string(),
                output_preview: "Build/Windows/x86_64/release/ComplexShooter".to_string(),
                dirty: false,
                validation_diagnostics: Vec::new(),
            }),
            commands: vec![
                BuildExportCommand::new("export_desktop_package", "Export", true, None),
                BuildExportCommand::new("build_release_package", "Build Release", true, None),
                BuildExportCommand::new("begin_asset_pick", "Pick Icon", true, None),
                BuildExportCommand::new("save_release_profile", "Save Profile", true, None),
                BuildExportCommand::new("open_build_output", "Output", true, None),
                BuildExportCommand::new("open_build_report", "Report", true, None),
            ],
            last_report: Some(BuildExportReportSummary {
                status: "success".to_string(),
                profile: "dev".to_string(),
                target: "windows".to_string(),
                package_dir: "D:/Projects/PlaneGame/Build/Windows/dev".to_string(),
                report_path:
                    "D:/Projects/PlaneGame/Build/Windows/dev/reports/desktop-export-report.json"
                        .to_string(),
                runtime_package_dir: "D:/Projects/PlaneGame/Build/Windows/dev/data/runtime_package"
                    .to_string(),
                player_exit_code: Some(0),
                player_exit_reason: "completed".to_string(),
                diagnostic_count: 0,
            }),
            last_release_report: None,
            empty_message: String::new(),
        },
        report_panel: editor_ui_model::ReportPanelModel::empty(),
        input_mapping_authoring: editor_ui_model::InputMappingAuthoringModel::empty(),
        rule_authoring: editor_ui_model::RuleAuthoringModel::empty(),
        project_authoring_workspace: editor_ui_model::ProjectAuthoringWorkspaceModel {
            project_root: Some("D:/Projects/PlaneGame".to_string()),
            project_id: Some("project-plane".to_string()),
            active_scene_id: Some("scene-main".to_string()),
            active_document: Some(editor_ui_model::WorkspaceDocumentSummary {
                document_kind: "scene".to_string(),
                document_id: Some("scene-main".to_string()),
                path: Some("Scenes/Main.scene.json".to_string()),
                dirty: false,
            }),
            selection: editor_ui_model::WorkspaceSelectionSummary {
                primary: Some(editor_ui_model::WorkspaceSelectionTarget::Entity {
                    entity_id: "entity-player".to_string(),
                }),
                secondary: Vec::new(),
            },
            domains: vec![
                editor_ui_model::WorkspaceDomainSummary::new(
                    editor_ui_model::WorkspaceDomainKind::Project,
                    "Project",
                    editor_ui_model::WorkspaceDomainStatus::Ready,
                    "project open",
                ),
                editor_ui_model::WorkspaceDomainSummary::new(
                    editor_ui_model::WorkspaceDomainKind::Scene,
                    "Scene",
                    editor_ui_model::WorkspaceDomainStatus::Ready,
                    "scene-main entity_count=1",
                ),
                editor_ui_model::WorkspaceDomainSummary::new(
                    editor_ui_model::WorkspaceDomainKind::Asset,
                    "Asset",
                    editor_ui_model::WorkspaceDomainStatus::Ready,
                    "asset_count=1",
                ),
                editor_ui_model::WorkspaceDomainSummary::new(
                    editor_ui_model::WorkspaceDomainKind::Build,
                    "Build",
                    editor_ui_model::WorkspaceDomainStatus::Ready,
                    "status=success",
                ),
            ],
            dirty_domains: Vec::new(),
            diagnostics: editor_ui_model::WorkspaceDiagnosticsSummary::default(),
            empty_message: String::new(),
            report: editor_ui_model::WorkspaceReportSummary {
                project_status: "open".to_string(),
                dirty_domains: Vec::new(),
                diagnostics: editor_ui_model::WorkspaceDiagnosticsSummary::default(),
                report_count: 0,
                evidence_count: 0,
                next_action_count: 0,
                last_command: None,
                last_transaction: None,
                build_status: Some("success".to_string()),
                play_status: Some("stopped".to_string()),
            },
        },
        authoring_workflow: editor_ui_model::AuthoringWorkflowModel {
            schema_version: editor_ui_model::AUTHORING_WORKFLOW_SCHEMA_VERSION.to_string(),
            project_id: Some("project-plane".to_string()),
            active_step: editor_ui_model::AuthoringStepId::Build,
            steps: vec![
                editor_ui_model::AuthoringWorkflowStep::new(
                    editor_ui_model::AuthoringStepId::Project,
                    editor_ui_model::AuthoringStepStatus::Ready,
                    editor_ui_model::AuthoringStepCompletion::Ready,
                ),
                editor_ui_model::AuthoringWorkflowStep::new(
                    editor_ui_model::AuthoringStepId::Scene,
                    editor_ui_model::AuthoringStepStatus::Ready,
                    editor_ui_model::AuthoringStepCompletion::Ready,
                ),
                with_primary_command(
                    editor_ui_model::AuthoringWorkflowStep::new(
                        editor_ui_model::AuthoringStepId::Build,
                        editor_ui_model::AuthoringStepStatus::Ready,
                        editor_ui_model::AuthoringStepCompletion::Ready,
                    ),
                    "export_desktop_package",
                    editor_ui_model::WorkspaceDomainKind::Build,
                    "Export",
                    "ExportDesktopPackage",
                ),
            ],
            global_status: editor_ui_model::AuthoringStepStatus::Ready,
            can_play: true,
            can_build: true,
            blocking_issues: Vec::new(),
            recommended_tasks: Vec::new(),
            ai_context: editor_ui_model::AuthoringAiContext {
                active_step: editor_ui_model::AuthoringStepId::Build,
                missing_required_items: Vec::new(),
                blocking_issues: Vec::new(),
                recommended_tasks: Vec::new(),
                available_commands: Vec::new(),
                manual_walkthrough_coverage: None,
                project_patch_summary: None,
                prefab_authoring_summary: None,
                aui_authoring_summary: None,
                summary: "status=Ready can_play=true can_build=true".to_string(),
            },
        },
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
            selected_entity_id: Some("entity-player".to_string()),
            roots: vec![editor_ui_model::HierarchyNode {
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
            frame: 3,
            frame_hash: Some("hash".to_string()),
            texture_id: Some("viewport-scene::frame-3".to_string()),
            target_id: Some("viewport-scene".to_string()),
            renderable_count: 1,
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

#[test]
fn ai_panel_draws_prompt_field_and_structured_submit_action() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("ai_panel"));

    assert!(draw_list
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AiPromptField)));
    assert!(draw_list.hit_regions.iter().any(|region| matches!(
        &region.target,
        HitTarget::AiPanelAction { action_id } if action_id == "submit:create an entity"
    )));
}

#[test]
fn ai_panel_narrow_dock_keeps_prompt_and_submit_separate() {
    let model = fixture_model();
    let mut list = UiDrawList {
        revision: 0,
        frame: 0,
        surface_width: 256.0,
        surface_height: 429.0,
        commands: Vec::new(),
        hit_regions: Vec::new(),
    };
    let interactions = crate::panels::push_ai_panel(
        &mut list,
        UiRect {
            x: 0.0,
            y: 28.0,
            width: 256.0,
            height: 401.0,
        },
        &model,
        &UiRendererConfig::new(1280.0, 720.0),
    );
    let rect = |id: &str| {
        let declaration = interactions
            .iter()
            .find(|declaration| declaration.id.as_str() == id)
            .expect("AI panel interaction");
        UiRect {
            x: declaration.style.inset_left.unwrap(),
            y: declaration.style.inset_top.unwrap(),
            width: declaration.style.width.unwrap(),
            height: declaration.style.height.unwrap(),
        }
    };
    let prompt = rect("editor/control/hit.ai_panel.prompt");
    let submit = rect("editor/control/hit.ai_panel.submit");

    assert!(prompt.width >= 40.0);
    assert!(prompt.x + prompt.width <= submit.x);
}

#[test]
fn gateway_access_rows_are_separate_from_ai_proposals_and_keep_unique_hit_ids() {
    let mut model = fixture_model();
    model.ai_panel.gateway_access = GatewayAccessInboxModel {
        requests: (0..2)
            .map(|index| GatewayAccessRequestModel {
                request_id: format!("gateway-access-request-{index}"),
                operation_short_id: format!("operation-{index}"),
                client_session_id: format!("gateway-session-{index}"),
                session_short_id: index.to_string(),
                client_kind: "MCP".to_string(),
                client_version: "codex-desktop.v1".to_string(),
                project_identity: "project.fixture".to_string(),
                connected_age_ms: 100,
                expires_in_ms: 10_000,
                state: "awaiting_user".to_string(),
                requested_profile: "project_owned_low_risk".to_string(),
                risk_class: "ProjectOwnedLowRisk".to_string(),
                capabilities: vec!["mutate_project".to_string()],
                blocked_capabilities: vec!["engine_core".to_string()],
                goal_id: format!("goal-{index}"),
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
            })
            .collect(),
        page_index: 0,
        page_count: 1,
        total_count: 2,
    };

    let draw_list = SelfUiRenderer::build_draw_list(&model, config_with_panel("ai_panel"));
    let gateway_hits = draw_list
        .hit_regions
        .iter()
        .filter(|region| matches!(region.target, HitTarget::GatewayAccessDecision { .. }))
        .collect::<Vec<_>>();
    let unique_ids = gateway_hits
        .iter()
        .map(|region| region.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        gateway_hits.len(),
        4,
        "two detailed rows need approve and reject actions"
    );
    assert_eq!(
        unique_ids.len(),
        4,
        "every Gateway action needs a unique hit id"
    );
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
}

#[test]
fn ai_panel_keeps_proposal_actions_inside_its_visible_surface() {
    let model = fixture_model();
    let mut renderer = RetainedEditorUiRenderer::default();
    let draw_list = renderer.build_draw_list(&model, config_with_panel("ai_panel"));
    let panel = renderer
        .tree()
        .expect("retained tree")
        .node(&WidgetId::semantic("editor/panel/ai").expect("AI panel widget id"))
        .expect("AI panel visible surface")
        .logical_rect;
    let accept = draw_list
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.ai_proposal.accept.proposal-1")
        .expect("AI proposal accept action")
        .rect;

    assert!(accept.x >= panel.x);
    assert!(accept.y >= panel.y);
    assert!(accept.x + accept.width <= panel.x + panel.width);
    assert!(accept.y + accept.height <= panel.y + panel.height);
}

#[test]
fn project_intent_panel_exposes_lifecycle_approval_and_run_actions() {
    let mut model = fixture_model();
    model.project_intent.intent.work_items = vec![
        editor_ui_model::ProjectIntentWorkItemModel {
            work_item_id: "work-active".to_string(),
            kind: "change".to_string(),
            title: "Adjust movement".to_string(),
            status: "ready".to_string(),
            ready: true,
            revision: 2,
        },
        editor_ui_model::ProjectIntentWorkItemModel {
            work_item_id: "work-parked".to_string(),
            kind: "idea".to_string(),
            title: "Online mode later".to_string(),
            status: "parked".to_string(),
            ready: false,
            revision: 1,
        },
        editor_ui_model::ProjectIntentWorkItemModel {
            work_item_id: "work-done".to_string(),
            kind: "bug".to_string(),
            title: "Regression".to_string(),
            status: "done".to_string(),
            ready: false,
            revision: 4,
        },
    ];
    model.project_intent.change_review.proposal_id = Some("change-set-1".to_string());
    model.project_intent.change_review.proposal_digest = Some("sha256:proposal".to_string());
    model.project_intent.change_review.approval_ready = true;
    model.project_intent.production.run_id = Some("run-1".to_string());
    model.project_intent.production.state = Some("executing".to_string());
    model.project_intent.production.total_steps = 2;

    let draw_list = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_active_bottom_panel(Some("project_intent".to_string())),
    );

    for (action_id, subject_id) in [
        ("park", "work-active"),
        ("resume", "work-parked"),
        ("reopen", "work-done"),
        ("approve", "sha256:proposal"),
        ("advance", "run-1"),
        ("cancel", "run-1"),
    ] {
        assert!(draw_list.hit_regions.iter().any(|region| matches!(
            &region.target,
            HitTarget::ProjectIntentAction { action_id: actual_action, subject_id: actual_subject }
                if actual_action == action_id && actual_subject == subject_id
        )));
    }
}

fn fixture_asset_browser_model() -> editor_ui_model::AssetBrowserModel {
    let scene = editor_ui_model::AssetBrowserEntry::authoring(
        "Scenes/Main.scene.json",
        "Main.scene.json",
        editor_ui_model::AssetKind::Scene,
        editor_ui_model::EditorAssetRef::new("scene-main", "scene"),
    );
    let folder = editor_ui_model::AssetBrowserEntry::new(
        "Scenes",
        "Scenes",
        editor_ui_model::AssetKind::Folder,
    );
    let mut model = editor_ui_model::AssetBrowserModel::empty();
    model.project_root = Some("D:/Projects/PlaneGame".to_string());
    model.index_status = editor_ui_model::AssetBrowserIndexStatus::Ready;
    model.scan_generation = 1;
    model.selection = editor_ui_model::AssetSelection::single_entry(&scene);
    model.folder_entries = vec![folder];
    model.entries = vec![scene];
    model
}

fn config_with_panel(panel_id: &str) -> UiRendererConfig {
    UiRendererConfig::new(1280.0, 720.0).with_active_bottom_panel(Some(panel_id.to_string()))
}

fn with_primary_command(
    mut step: editor_ui_model::AuthoringWorkflowStep,
    command_id: &str,
    domain: editor_ui_model::WorkspaceDomainKind,
    label: &str,
    payload_kind: &str,
) -> editor_ui_model::AuthoringWorkflowStep {
    step.primary_command = Some(editor_ui_model::AuthoringCommand::new(
        command_id,
        domain,
        label,
        editor_ui_model::AuthoringCommandAvailability::Available,
        payload_kind,
    ));
    step
}
