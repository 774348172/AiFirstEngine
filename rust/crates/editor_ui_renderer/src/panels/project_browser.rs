use editor_ui_model::{
    AssetBrowserEntry, AssetBrowserIndexStatus, AssetBrowserToolbarAction, AssetBrowserViewMode,
    AssetEntryRole, AssetKind, EditorUiModel,
};

use crate::layout::push_border;
use crate::{
    ActivationPolicy, ControlPseudoStateSet, DrawCommand, EditorCommandBinding, EditorWidgetAction,
    EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetId,
    WidgetPaint, WidgetRole,
};

pub(crate) fn push_project_browser(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
    config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let panel = UiRect {
        x: rect.x + rect.width * 0.74,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width * 0.26 - 1.0,
        height: rect.height - 25.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    push_border(list, panel);

    let browser = &model.asset_browser;
    push_header(list, panel, browser);
    push_toolbar(list, panel, browser, &mut interactions);

    let body_top = panel.y + 67.0;
    let status_height = 20.0;
    let body_height = (panel.height - 67.0 - status_height).max(0.0);
    let folder_width = (panel.width * 0.26).max(76.0);
    let preview_width = (panel.width * 0.25).max(82.0);
    let asset_width = (panel.width - folder_width - preview_width - 4.0).max(90.0);
    let folder_rect = UiRect {
        x: panel.x + 2.0,
        y: body_top,
        width: folder_width,
        height: body_height,
    };
    let asset_rect = UiRect {
        x: folder_rect.x + folder_rect.width + 2.0,
        y: body_top,
        width: asset_width,
        height: body_height,
    };
    let preview_rect = UiRect {
        x: asset_rect.x + asset_rect.width + 2.0,
        y: body_top,
        width: preview_width,
        height: body_height,
    };

    push_subpanel_background(list, folder_rect);
    push_subpanel_background(list, asset_rect);
    push_subpanel_background(list, preview_rect);
    push_folder_tree(list, folder_rect, browser, &mut interactions);
    let command_start = list.commands.len();
    let interaction_start = interactions.len();
    push_asset_body(list, asset_rect, browser, &mut interactions);
    let body_commands = list.commands.drain(command_start..).collect::<Vec<_>>();
    let body_interactions = interactions.split_off(interaction_start);
    interactions.push(scroll_subtree(
        "editor/panel/asset-browser/assets",
        asset_rect,
        body_commands,
        body_interactions,
    ));
    push_preview(list, preview_rect, browser);
    push_status(list, panel, browser, config, &mut interactions);
    interactions
}

fn push_header(list: &mut UiDrawList, panel: UiRect, browser: &editor_ui_model::AssetBrowserModel) {
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 8.0,
            y: panel.y + 6.0,
            width: panel.width - 16.0,
            height: 14.0,
        },
        text: format!(
            "{}  {}",
            if browser.picker.is_some() {
                "Pick Asset"
            } else {
                "Assets"
            },
            index_status_label(browser.index_status)
        ),
        color: if matches!(
            browser.index_status,
            AssetBrowserIndexStatus::Stale | AssetBrowserIndexStatus::Failed
        ) {
            UiColor::WARNING
        } else {
            UiColor::TEXT
        },
        size: 11.0,
    });
}

fn push_toolbar(
    list: &mut UiDrawList,
    panel: UiRect,
    browser: &editor_ui_model::AssetBrowserModel,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let y = panel.y + 23.0;
    let mut x = panel.x + 5.0;
    for (label, action) in [
        ("<", AssetBrowserToolbarAction::Back),
        (">", AssetBrowserToolbarAction::Forward),
        ("^", AssetBrowserToolbarAction::Up),
    ] {
        push_toolbar_button(list, x, y, 19.0, label, action, interactions);
        x += 21.0;
    }

    let breadcrumb_width = (panel.width - 188.0).max(54.0);
    let breadcrumb = UiRect {
        x,
        y,
        width: breadcrumb_width,
        height: 18.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: breadcrumb,
        color: UiColor::FIELD,
        corner_radius: 2.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: inset(breadcrumb, 5.0, 3.0),
        text: browser
            .current_folder
            .clone()
            .unwrap_or_else(|| "/".to_string()),
        color: UiColor::TEXT,
        size: 9.0,
    });
    x += breadcrumb_width + 3.0;
    push_toolbar_button(
        list,
        x,
        y,
        21.0,
        "R",
        AssetBrowserToolbarAction::Refresh,
        interactions,
    );
    x += 23.0;
    push_toolbar_button(
        list,
        x,
        y,
        21.0,
        match browser.view_mode {
            AssetBrowserViewMode::List => "L",
            AssetBrowserViewMode::Grid => "G",
        },
        AssetBrowserToolbarAction::ToggleView,
        interactions,
    );
    x += 23.0;
    push_toolbar_button(
        list,
        x,
        y,
        28.0,
        if browser.query.kinds.is_empty() {
            "All"
        } else {
            "Type"
        },
        AssetBrowserToolbarAction::CycleTypeFilter,
        interactions,
    );

    let search = UiRect {
        x: panel.x + 5.0,
        y: y + 21.0,
        width: panel.width - 35.0,
        height: 18.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: search,
        color: UiColor::FIELD,
        corner_radius: 2.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: inset(search, 5.0, 3.0),
        text: if browser.query.search_text.is_empty() {
            "Search".to_string()
        } else {
            browser.query.search_text.clone()
        },
        color: if browser.query.search_text.is_empty() {
            UiColor::TEXT_MUTED
        } else {
            UiColor::TEXT
        },
        size: 9.0,
    });
    push_interaction(
        interactions,
        AssetInteractionSpec {
            hit_id: "hit.asset_browser.search".to_string(),
            rect: search,
            role: WidgetRole::TextInput,
            target: HitTarget::AssetBrowserSearch,
            enabled: true,
            command_id: "focus_asset_browser_search",
            reason_disabled: None,
        },
    );
    push_toolbar_button(
        list,
        panel.x + panel.width - 27.0,
        y + 21.0,
        22.0,
        "X",
        AssetBrowserToolbarAction::ClearSearch,
        interactions,
    );
}

fn push_toolbar_button(
    list: &mut UiDrawList,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    action: AssetBrowserToolbarAction,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let rect = UiRect {
        x,
        y,
        width,
        height: 18.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::PANEL_LIGHT,
        corner_radius: 2.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: inset(rect, 4.0, 3.0),
        text: label.to_string(),
        color: UiColor::TEXT,
        size: 9.0,
    });
    push_interaction(
        interactions,
        AssetInteractionSpec {
            hit_id: format!("hit.asset_browser.action.{action:?}"),
            rect,
            role: WidgetRole::Button,
            target: HitTarget::AssetBrowserAction { action },
            enabled: true,
            command_id: "asset_browser_toolbar",
            reason_disabled: None,
        },
    );
}

fn push_folder_tree(
    list: &mut UiDrawList,
    rect: UiRect,
    browser: &editor_ui_model::AssetBrowserModel,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let mut y = rect.y + 4.0;
    for entry in &browser.folder_entries {
        if !entry.exists || y + 18.0 > rect.y + rect.height {
            continue;
        }
        let depth = entry.path.matches('/').count().min(3) as f32;
        let row = UiRect {
            x: rect.x + 2.0,
            y,
            width: rect.width - 4.0,
            height: 18.0,
        };
        let active = browser.current_folder.as_deref() == Some(entry.path.as_str());
        list.commands.push(DrawCommand::Rect {
            rect: row,
            color: if active {
                UiColor::TAB_ACTIVE
            } else {
                UiColor::PANEL
            },
            corner_radius: 0.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 4.0 + depth * 7.0,
                y: row.y + 4.0,
                width: (row.width - 8.0 - depth * 7.0).max(8.0),
                height: 11.0,
            },
            text: format!("+ {}", entry.label),
            color: UiColor::TEXT,
            size: 8.0,
        });
        push_interaction(
            interactions,
            AssetInteractionSpec {
                hit_id: format!(
                    "hit.asset_browser.folder.{}",
                    entry.entry_key.stable_token()
                ),
                rect: row,
                role: WidgetRole::Button,
                target: HitTarget::AssetBrowserFolder {
                    path: entry.path.clone(),
                },
                enabled: true,
                command_id: "set_asset_browser_folder",
                reason_disabled: None,
            },
        );
        y += 19.0;
    }
}

fn push_asset_body(
    list: &mut UiDrawList,
    rect: UiRect,
    browser: &editor_ui_model::AssetBrowserModel,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let entries = browser
        .entries
        .iter()
        .filter(|entry| entry.role != AssetEntryRole::Folder)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        list.commands.push(DrawCommand::Text {
            rect: inset(rect, 6.0, 7.0),
            text: match browser.index_status {
                AssetBrowserIndexStatus::Scanning => "Scanning assets...".to_string(),
                AssetBrowserIndexStatus::Failed => "Asset scan failed.".to_string(),
                _ => browser.empty_message.clone(),
            },
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        return;
    }
    match browser.view_mode {
        AssetBrowserViewMode::List => push_asset_list(list, rect, browser, &entries, interactions),
        AssetBrowserViewMode::Grid => push_asset_grid(list, rect, browser, &entries, interactions),
    }
}

fn push_asset_list(
    list: &mut UiDrawList,
    rect: UiRect,
    _browser: &editor_ui_model::AssetBrowserModel,
    entries: &[&AssetBrowserEntry],
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let row_height = 22.0;
    let mut y = rect.y + 2.0;
    for entry in entries {
        let row = UiRect {
            x: rect.x + 2.0,
            y,
            width: rect.width - 4.0,
            height: row_height - 2.0,
        };
        push_asset_entry(list, row, entry, false, interactions);
        y += row_height;
    }
}

fn push_asset_grid(
    list: &mut UiDrawList,
    rect: UiRect,
    browser: &editor_ui_model::AssetBrowserModel,
    entries: &[&AssetBrowserEntry],
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let tile_width = (browser.thumbnail_size as f32 + 16.0).clamp(68.0, 116.0);
    let tile_height = tile_width + 18.0;
    let columns = ((rect.width - 4.0) / tile_width).floor().max(1.0) as usize;
    for (visible_index, entry) in entries.iter().enumerate() {
        let column = visible_index % columns;
        let row_index = visible_index / columns;
        let tile = UiRect {
            x: rect.x + 2.0 + column as f32 * tile_width,
            y: rect.y + 2.0 + row_index as f32 * tile_height,
            width: tile_width - 3.0,
            height: tile_height - 3.0,
        };
        push_asset_entry(list, tile, entry, true, interactions);
    }
}

fn scroll_subtree(
    id: &str,
    rect: UiRect,
    commands: Vec<DrawCommand>,
    mut interactions: Vec<EditorWidgetDeclaration>,
) -> EditorWidgetDeclaration {
    let mut root = EditorWidgetDeclaration::new(
        WidgetId::semantic(id).expect("static asset browser scroll id"),
        WidgetRole::Scroll,
    )
    .with_absolute_rect(rect, 74_000);
    root.style.clip = true;
    for (index, command) in commands.into_iter().enumerate() {
        let unclipped = command.unclipped();
        let command_rect = draw_command_rect(unclipped);
        let mut declaration = EditorWidgetDeclaration::new(
            WidgetId::semantic(format!("{id}/paint/{index}"))
                .expect("generated asset browser paint id"),
            draw_command_role(unclipped),
        )
        .with_absolute_rect(
            UiRect {
                x: command_rect.x - rect.x,
                y: command_rect.y - rect.y,
                width: command_rect.width,
                height: command_rect.height,
            },
            index as i32,
        );
        declaration.paint.push(draw_command_paint(unclipped));
        root.children.push(declaration);
    }
    for interaction in &mut interactions {
        interaction.style.inset_left = interaction.style.inset_left.map(|x| x - rect.x);
        interaction.style.inset_top = interaction.style.inset_top.map(|y| y - rect.y);
    }
    root.children.extend(interactions);
    root
}

fn draw_command_rect(command: &DrawCommand) -> UiRect {
    match command {
        DrawCommand::Rect { rect, .. }
        | DrawCommand::Text { rect, .. }
        | DrawCommand::ViewportTextureSlot { rect, .. }
        | DrawCommand::ImageTextureSlot { rect, .. } => *rect,
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    }
}

fn draw_command_role(command: &DrawCommand) -> WidgetRole {
    match command {
        DrawCommand::Text { .. } => WidgetRole::Label,
        DrawCommand::ImageTextureSlot { .. } => WidgetRole::Image,
        DrawCommand::ViewportTextureSlot { .. } => WidgetRole::Viewport,
        DrawCommand::Rect { .. } => WidgetRole::Container,
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    }
}

fn draw_command_paint(command: &DrawCommand) -> WidgetPaint {
    match command {
        DrawCommand::Rect {
            color,
            corner_radius,
            ..
        } => WidgetPaint::Rect {
            color: *color,
            corner_radius: *corner_radius,
        },
        DrawCommand::Text {
            text, color, size, ..
        } => WidgetPaint::Text {
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
        } => WidgetPaint::Image {
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
        } => WidgetPaint::Viewport {
            scene_id: scene_id.clone(),
            frame: *frame,
            texture_id: texture_id.clone(),
            target_id: target_id.clone(),
        },
        DrawCommand::Clipped { .. } => unreachable!("command was normalized"),
    }
}

fn push_asset_entry(
    list: &mut UiDrawList,
    rect: UiRect,
    entry: &AssetBrowserEntry,
    grid: bool,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: if entry.selected {
            UiColor::ACCENT
        } else if entry.exists && entry.imported {
            UiColor::PANEL
        } else {
            UiColor::PANEL_DARK
        },
        corner_radius: 2.0,
    });
    let has_thumbnail = push_thumbnail_slot(list, rect, entry, grid);
    let label_rect = if grid {
        UiRect {
            x: rect.x + 3.0,
            y: rect.y + rect.height - 17.0,
            width: rect.width - 6.0,
            height: 13.0,
        }
    } else {
        UiRect {
            x: rect.x + if has_thumbnail { 24.0 } else { 5.0 },
            y: rect.y + 4.0,
            width: rect.width - if has_thumbnail { 42.0 } else { 23.0 },
            height: 12.0,
        }
    };
    list.commands.push(DrawCommand::Text {
        rect: label_rect,
        text: format!("{} {}", kind_label(entry.kind), entry.label),
        color: if entry.exists {
            UiColor::TEXT
        } else {
            UiColor::TEXT_MUTED
        },
        size: if grid { 8.0 } else { 9.0 },
    });
    let token = entry.entry_key.stable_token();
    push_interaction(
        interactions,
        AssetInteractionSpec {
            hit_id: format!("hit.asset_browser.entry.{token}"),
            rect,
            role: WidgetRole::Button,
            target: HitTarget::AssetBrowserEntry {
                entry_key: entry.entry_key.clone(),
                path: entry.path.clone(),
            },
            enabled: entry.selectable,
            command_id: "select_asset_browser_entry",
            reason_disabled: (!entry.selectable)
                .then(|| "Asset entry is not selectable.".to_string()),
        },
    );
    if !grid && entry.openable && entry.exists {
        let open = UiRect {
            x: rect.x + rect.width - 18.0,
            y: rect.y + 2.0,
            width: 16.0,
            height: 16.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: inset(open, 4.0, 3.0),
            text: ">".to_string(),
            color: UiColor::TEXT,
            size: 9.0,
        });
        push_interaction(
            interactions,
            AssetInteractionSpec {
                hit_id: format!("hit.asset_browser.open.{token}"),
                rect: open,
                role: WidgetRole::Button,
                target: HitTarget::AssetBrowserOpen {
                    entry_key: entry.entry_key.clone(),
                    path: entry.path.clone(),
                },
                enabled: true,
                command_id: "open_asset_browser_entry",
                reason_disabled: None,
            },
        );
    }
}

fn push_thumbnail_slot(
    list: &mut UiDrawList,
    rect: UiRect,
    entry: &AssetBrowserEntry,
    grid: bool,
) -> bool {
    if entry.preview.preview_kind != editor_ui_model::AssetPreviewKind::Thumbnail {
        return false;
    }
    let image_bounds = if grid {
        UiRect {
            x: rect.x + 4.0,
            y: rect.y + 4.0,
            width: (rect.width - 8.0).max(1.0),
            height: (rect.height - 24.0).max(1.0),
        }
    } else {
        UiRect {
            x: rect.x + 3.0,
            y: rect.y + 2.0,
            width: 18.0,
            height: 16.0,
        }
    };
    let image_rect = fit_aspect_ratio(image_bounds, entry.preview.thumbnail_aspect_ratio);
    list.commands.push(DrawCommand::ImageTextureSlot {
        rect: image_rect,
        source_uv: crate::UiUvRect::FULL,
        texture_id: entry.preview.thumbnail_id.clone(),
        fallback_color: thumbnail_fallback_color(entry.preview.status),
        tint: UiColor::IDENTITY_TINT,
    });
    true
}

fn push_preview(list: &mut UiDrawList, rect: UiRect, browser: &editor_ui_model::AssetBrowserModel) {
    let selected = browser
        .selection
        .primary_entry_key
        .as_ref()
        .and_then(|key| browser.entries.iter().find(|entry| &entry.entry_key == key));
    let Some(entry) = selected else {
        list.commands.push(DrawCommand::Text {
            rect: inset(rect, 5.0, 6.0),
            text: "No selection".to_string(),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        return;
    };
    let mut y = rect.y + 6.0;
    if entry.preview.preview_kind == editor_ui_model::AssetPreviewKind::Thumbnail {
        let image_bounds = UiRect {
            x: rect.x + 5.0,
            y,
            width: (rect.width - 10.0).max(1.0),
            height: (rect.height * 0.42).clamp(42.0, 140.0),
        };
        let image_rect = fit_aspect_ratio(image_bounds, entry.preview.thumbnail_aspect_ratio);
        list.commands.push(DrawCommand::ImageTextureSlot {
            rect: image_rect,
            source_uv: crate::UiUvRect::FULL,
            texture_id: entry.preview.thumbnail_id.clone(),
            fallback_color: thumbnail_fallback_color(entry.preview.status),
            tint: UiColor::IDENTITY_TINT,
        });
        y = image_bounds.y + image_bounds.height + 6.0;
    }
    let lines = [
        entry.label.clone(),
        format!("{:?}", entry.kind),
        entry
            .asset_id
            .clone()
            .unwrap_or_else(|| "Source file".to_string()),
        format!("{:?}", entry.identity_status),
        format!("{:?}", entry.source_status),
    ];
    for (index, line) in lines.into_iter().enumerate() {
        if y + 13.0 > rect.y + rect.height {
            break;
        }
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + 5.0,
                y,
                width: rect.width - 10.0,
                height: 12.0,
            },
            text: line,
            color: if index == 0 {
                UiColor::TEXT
            } else {
                UiColor::TEXT_MUTED
            },
            size: if index == 0 { 9.0 } else { 8.0 },
        });
        y += 15.0;
    }
}

fn fit_aspect_ratio(
    bounds: UiRect,
    aspect_ratio: Option<editor_ui_model::AssetThumbnailAspectRatio>,
) -> UiRect {
    let Some(aspect_ratio) = aspect_ratio.map(|ratio| ratio.as_f32()) else {
        return bounds;
    };
    if !aspect_ratio.is_finite()
        || aspect_ratio <= 0.0
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return bounds;
    }
    let bounds_ratio = bounds.width / bounds.height;
    if aspect_ratio > bounds_ratio {
        let height = bounds.width / aspect_ratio;
        UiRect {
            y: bounds.y + (bounds.height - height) * 0.5,
            height,
            ..bounds
        }
    } else {
        let width = bounds.height * aspect_ratio;
        UiRect {
            x: bounds.x + (bounds.width - width) * 0.5,
            width,
            ..bounds
        }
    }
}

fn thumbnail_fallback_color(status: editor_ui_model::AssetPreviewStatus) -> UiColor {
    match status {
        editor_ui_model::AssetPreviewStatus::Failed => {
            crate::EditorTheme::DARK_NEUTRAL.status.error_surface
        }
        editor_ui_model::AssetPreviewStatus::NotAvailable => UiColor::PANEL_DARK,
        editor_ui_model::AssetPreviewStatus::Pending => {
            crate::EditorTheme::DARK_NEUTRAL.status.pending_surface
        }
        editor_ui_model::AssetPreviewStatus::Ready => {
            crate::EditorTheme::DARK_NEUTRAL.status.ready_surface
        }
    }
}

fn push_status(
    list: &mut UiDrawList,
    panel: UiRect,
    browser: &editor_ui_model::AssetBrowserModel,
    config: &UiRendererConfig,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let rect = UiRect {
        x: panel.x + 4.0,
        y: panel.y + panel.height - 18.0,
        width: panel.width - if browser.picker.is_some() { 76.0 } else { 8.0 },
        height: 14.0,
    };
    list.commands.push(DrawCommand::Text {
        rect,
        text: format!(
            "{} visible  {} selected  gen {}  {}",
            browser.entries.len(),
            browser.selection.selected_entry_keys.len(),
            browser.scan_generation,
            index_status_label(browser.index_status)
        ),
        color: UiColor::TEXT_MUTED,
        size: 8.0,
    });
    if let Some(picker) = &browser.picker {
        let confirm = UiRect {
            x: panel.x + panel.width - 68.0,
            y: panel.y + panel.height - 21.0,
            width: 30.0,
            height: 18.0,
        };
        let cancel = UiRect {
            x: panel.x + panel.width - 34.0,
            y: confirm.y,
            width: 30.0,
            height: 18.0,
        };
        for (button, label, enabled, hit_id) in [
            (
                confirm,
                "OK",
                picker.can_confirm,
                "hit.asset_picker.confirm",
            ),
            (cancel, "X", true, "hit.asset_picker.cancel"),
        ] {
            let style = super::resolve_and_paint_control(
                list,
                button,
                WidgetRole::Button,
                "decision-control",
                config.control_pseudo_states(hit_id, ControlPseudoStateSet::empty(), enabled),
            );
            list.commands.push(DrawCommand::Text {
                rect: inset(button, 7.0, 3.0),
                text: label.to_string(),
                color: style.foreground,
                size: 8.0,
            });
        }
        push_interaction(
            interactions,
            AssetInteractionSpec {
                hit_id: "hit.asset_picker.confirm".to_string(),
                rect: confirm,
                role: WidgetRole::Button,
                target: HitTarget::AssetPickerConfirm,
                enabled: picker.can_confirm,
                command_id: "confirm_asset_pick",
                reason_disabled: (!picker.can_confirm)
                    .then(|| "Select a compatible asset first.".to_string()),
            },
        );
        push_interaction(
            interactions,
            AssetInteractionSpec {
                hit_id: "hit.asset_picker.cancel".to_string(),
                rect: cancel,
                role: WidgetRole::Button,
                target: HitTarget::AssetPickerCancel,
                enabled: true,
                command_id: "cancel_asset_pick",
                reason_disabled: None,
            },
        );
    }
}

struct AssetInteractionSpec {
    hit_id: String,
    rect: UiRect,
    role: WidgetRole,
    target: HitTarget,
    enabled: bool,
    command_id: &'static str,
    reason_disabled: Option<String>,
}

fn push_interaction(interactions: &mut Vec<EditorWidgetDeclaration>, spec: AssetInteractionSpec) {
    let decision_control = matches!(
        &spec.target,
        HitTarget::AssetPickerConfirm | HitTarget::AssetPickerCancel
    );
    let mut declaration = EditorWidgetDeclaration::new(
        WidgetId::scoped("editor/panel/asset-browser/control", &spec.hit_id)
            .expect("static asset browser scope"),
        spec.role,
    )
    .with_absolute_rect(spec.rect, 75_000 + interactions.len() as i32)
    .with_interaction(
        spec.hit_id,
        spec.enabled,
        EditorCommandBinding {
            action: if spec.role == WidgetRole::TextInput {
                EditorWidgetAction::Focus
            } else {
                EditorWidgetAction::Activate
            },
            command_id: spec.command_id.to_string(),
            target: spec.target,
            reason_disabled: spec.reason_disabled,
        },
    );
    if decision_control {
        declaration.control_classes = crate::ControlClassSet::new(["decision-control"]);
        declaration.activation_policy = ActivationPolicy::ReleaseInside;
    }
    interactions.push(declaration);
}

fn push_subpanel_background(list: &mut UiDrawList, rect: UiRect) {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::PANEL,
        corner_radius: 0.0,
    });
}

fn kind_label(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Scene => "S",
        AssetKind::Prefab => "P",
        AssetKind::Texture => "T",
        AssetKind::Sprite => "Sp",
        AssetKind::Material => "M",
        AssetKind::Rule => "R",
        AssetKind::Aui => "UI",
        AssetKind::InputMapping => "In",
        AssetKind::Font => "F",
        AssetKind::Audio => "A",
        AssetKind::BuildProfile => "B",
        AssetKind::ProjectSettings => "Cfg",
        AssetKind::Folder => "+",
        AssetKind::Unknown => "?",
    }
}

fn index_status_label(status: AssetBrowserIndexStatus) -> &'static str {
    match status {
        AssetBrowserIndexStatus::NotBuilt => "Not built",
        AssetBrowserIndexStatus::Scanning => "Scanning",
        AssetBrowserIndexStatus::Ready => "Ready",
        AssetBrowserIndexStatus::Stale => "Stale",
        AssetBrowserIndexStatus::Failed => "Failed",
    }
}

fn inset(rect: UiRect, x: f32, y: f32) -> UiRect {
    UiRect {
        x: rect.x + x,
        y: rect.y + y,
        width: (rect.width - x * 2.0).max(0.0),
        height: (rect.height - y * 2.0).max(0.0),
    }
}
