use super::*;

impl AuiLayoutEngine {
    pub fn validate(
        document: &AuiDocument,
        manifest: Option<&AuiAssetManifest>,
    ) -> AuiValidationReport {
        let node_ids: HashSet<&str> = document
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect();
        let mut seen = HashSet::new();
        let asset_ids = manifest
            .map(AuiAssetManifest::asset_ids)
            .unwrap_or_default();
        let mut items = crate::aui_layout_schema::validate(document);
        let mut missing_asset_count = 0;

        for node in &document.nodes {
            if !seen.insert(node.node_id.as_str()) {
                items.push(AuiValidationItem::error(
                    "duplicate_node_id",
                    Some(node.node_id.clone()),
                    None,
                    format!("AUI node '{}' is duplicated.", node.node_id),
                    "Keep node_id unique inside one AuiDocument.",
                ));
            }

            match node.kind {
                AuiNodeKind::Image if node.text.is_some() => {
                    items.push(AuiValidationItem::warning(
                        "image_node_text_field_ignored",
                        Some(node.node_id.clone()),
                        None,
                        format!(
                            "AUI Image node '{}' contains text, but Image nodes generate DrawImage only.",
                            node.node_id
                        ),
                        "Move text into a Text child node inside the same AUI subtree.",
                    ));
                }
                AuiNodeKind::Text if node.image.is_some() => {
                    items.push(AuiValidationItem::warning(
                        "text_node_image_field_ignored",
                        Some(node.node_id.clone()),
                        node.image.as_ref().map(|image| image.asset_id.clone()),
                        format!(
                            "AUI Text node '{}' contains image, but Text nodes generate DrawText only.",
                            node.node_id
                        ),
                        "Move image into an Image child node inside the same AUI subtree.",
                    ));
                }
                _ => {}
            }

            if let Some(parent) = &node.parent {
                if !node_ids.contains(parent.as_str()) {
                    items.push(AuiValidationItem::error(
                        "missing_parent",
                        Some(node.node_id.clone()),
                        None,
                        format!(
                            "AUI node '{}' references missing parent '{}'.",
                            node.node_id, parent
                        ),
                        "Create the parent node or update the node parent field.",
                    ));
                }
            }

            if let Some(image) = &node.image {
                if !asset_ids.contains(image.asset_id.as_str()) {
                    missing_asset_count += 1;
                    items.push(AuiValidationItem::error(
                        "missing_image_asset",
                        Some(node.node_id.clone()),
                        Some(image.asset_id.clone()),
                        format!(
                            "AUI image node '{}' references missing asset '{}'.",
                            node.node_id, image.asset_id
                        ),
                        "Add the asset to AuiAssetManifest or update node.image.",
                    ));
                }
            }
        }

        for canvas in &document.canvases {
            if !node_ids.contains(canvas.root_node.as_str()) {
                items.push(AuiValidationItem::error(
                    "missing_canvas_root",
                    Some(canvas.root_node.clone()),
                    None,
                    format!(
                        "AUI canvas '{}' references missing root node '{}'.",
                        canvas.canvas_id, canvas.root_node
                    ),
                    "Create the canvas root node or update canvas.root_node.",
                ));
            }
        }

        let full_screen_image_rejected = Self::is_full_screen_single_image_ui(document);
        if full_screen_image_rejected {
            items.push(AuiValidationItem::error(
                "full_screen_image_ui_rejected",
                None,
                None,
                "AUI document looks like a full-screen image UI shortcut.",
                "Split the UI into AuiNode Image/Text/Button structures instead of using one screenshot.",
            ));
        }

        AuiValidationReport::from_items(items, missing_asset_count, full_screen_image_rejected)
    }

    pub fn layout(document: &AuiDocument, frame: u64) -> AuiLayoutResult {
        let scroll_offsets = BTreeMap::new();
        Self::layout_with_scroll_offsets(document, frame, &scroll_offsets)
    }

    pub fn layout_with_scroll_offsets(
        document: &AuiDocument,
        frame: u64,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
    ) -> AuiLayoutResult {
        Self::layout_with_scroll_offsets_and_canvas_visibility(
            document,
            frame,
            scroll_offsets,
            &BTreeMap::new(),
        )
    }

    pub fn layout_with_interaction_state(
        document: &AuiDocument,
        frame: u64,
        state: &AuiInteractionState,
    ) -> AuiLayoutResult {
        Self::layout_with_scroll_offsets_and_canvas_visibility(
            document,
            frame,
            &state.scroll_offsets,
            &state.canvas_visibility_overrides,
        )
    }

    pub(super) fn layout_with_scroll_offsets_and_canvas_visibility(
        document: &AuiDocument,
        frame: u64,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
        canvas_visibility_overrides: &BTreeMap<String, bool>,
    ) -> AuiLayoutResult {
        let mut result =
            Self::layout_raw(document, frame, scroll_offsets, canvas_visibility_overrides);
        if document.schema_version != crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION {
            apply_retained_translations(document, &mut result, &HashMap::new(), &HashMap::new());
        }
        result
    }

    pub(super) fn layout_raw(
        document: &AuiDocument,
        frame: u64,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
        canvas_visibility_overrides: &BTreeMap<String, bool>,
    ) -> AuiLayoutResult {
        if document.schema_version == crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION {
            let mut runtime = AuiC2Runtime::default();
            let (mut result, _) =
                runtime.layout(document, frame, &RuntimeFontBundleRegistry::default());
            for computed in &mut result.computed_nodes {
                if canvas_visibility_overrides
                    .get(&computed.canvas_id)
                    .is_some_and(|visible| !visible)
                {
                    computed.effective_visible = false;
                }
            }
            result.report.visible_node_count = result
                .computed_nodes
                .iter()
                .filter(|node| node.effective_visible)
                .count();
            return result;
        }
        if crate::aui_layout_schema::uses_c2(document)
            || !crate::aui_layout_schema::supported_schema(&document.schema_version)
        {
            let diagnostics = crate::aui_layout_schema::validate(document);
            return AuiLayoutResult {
                diagnostics,
                report: AuiLayoutReport {
                    frame,
                    ..Default::default()
                },
                ..Default::default()
            };
        }
        let nodes_by_id: HashMap<&str, &AuiNode> = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect();
        let mut computed_nodes = Vec::new();
        let mut tree_order = 0;
        let mut scroll_offset_applied = false;
        let mut scroll_applied_node_count = 0;
        let mut clipped_node_count = 0;
        let mut clip_root_count = 0;

        for canvas in &document.canvases {
            if canvas.mode != AuiCanvasMode::ScreenOverlay {
                continue;
            }
            let visible = canvas_visibility_overrides
                .get(canvas.canvas_id.as_str())
                .copied()
                .unwrap_or(canvas.visible);
            if !visible {
                continue;
            }
            let root_rect = AuiComputedRect {
                x: 0.0,
                y: 0.0,
                width: canvas.reference_resolution.x,
                height: canvas.reference_resolution.y,
            };
            Self::layout_node(
                canvas,
                &nodes_by_id,
                canvas.root_node.as_str(),
                root_rect,
                &mut tree_order,
                &mut computed_nodes,
                scroll_offsets,
                None,
                &mut scroll_offset_applied,
                &mut scroll_applied_node_count,
                &mut clipped_node_count,
                &mut clip_root_count,
                true,
            );
        }

        let visible_node_count = computed_nodes
            .iter()
            .filter(|node| node.effective_visible)
            .count();
        let scrollbar_metrics =
            Self::compute_scrollbar_metrics(&nodes_by_id, &computed_nodes, scroll_offsets);
        AuiLayoutResult {
            diagnostics: Vec::new(),
            scrollbar_metrics,
            report: AuiLayoutReport {
                frame,
                canvas_count: document.canvases.len(),
                node_count: computed_nodes.len(),
                visible_node_count,
                clipped_node_count,
                clip_root_count,
                effective_clip_node_count: computed_nodes
                    .iter()
                    .filter(|node| node.effective_clip_rect.is_some())
                    .count(),
                overflow_count: 0,
                invalid_binding_count: 0,
                scroll_offset_applied,
                scroll_applied_node_count,
            },
            computed_nodes,
            interaction_index_update: Default::default(),
        }
    }

    pub fn extract_draw_list(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
    ) -> (AuiDrawList, AuiRenderReport) {
        Self::extract_draw_list_internal(document, layout, None, None)
    }

    pub fn extract_draw_list_with_visual_overrides(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        overrides: &crate::aui_control_feedback::AuiVisualOverrideSet,
    ) -> (AuiDrawList, AuiRenderReport) {
        Self::extract_draw_list_internal(document, layout, Some(overrides), None)
    }

    pub(super) fn extract_draw_list_cached(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        overrides: Option<&crate::aui_control_feedback::AuiVisualOverrideSet>,
        cache: &mut AuiDrawingCache,
    ) -> (AuiDrawList, AuiRenderReport) {
        Self::extract_draw_list_internal(document, layout, overrides, Some(cache))
    }

    pub(super) fn extract_draw_list_internal(
        document: &AuiDocument,
        layout: &AuiLayoutResult,
        overrides: Option<&crate::aui_control_feedback::AuiVisualOverrideSet>,
        mut cache: Option<&mut AuiDrawingCache>,
    ) -> (AuiDrawList, AuiRenderReport) {
        let mut standalone_graphics = crate::aui_graphics::GraphicsCache::default();
        let retain =
            cache.is_some() && layout.computed_nodes.len() <= drawing_cache::MAX_DRAW_NODES;
        let owner_changes = cache
            .as_mut()
            .and_then(|cache| cache.draw_changes_once.take());
        let changed_slots = if retain && owner_changes.is_some() {
            owner_changes
        } else if retain && overrides.is_none() {
            cache
                .as_ref()
                .and_then(|cache| cache.changed_layout_slots(layout))
        } else {
            None
        };
        let incremental = changed_slots.is_some();
        let node_overrides = cache
            .as_mut()
            .map(|cache| std::mem::take(&mut cache.draw_node_overrides))
            .unwrap_or_default();
        let mut fragments = if retain {
            let previous = cache
                .as_mut()
                .map(|cache| std::mem::take(&mut cache.nodes))
                .unwrap_or_default();
            if previous.len() == layout.computed_nodes.len() {
                previous
            } else {
                vec![None; layout.computed_nodes.len()]
            }
        } else {
            Vec::new()
        };
        let mut indices = cache
            .as_mut()
            .map(|c| std::mem::take(&mut c.indices))
            .unwrap_or_default();
        if !incremental {
            if let Some(cache) = cache.as_mut() {
                cache.graphics_errors.retain(|key, _| {
                    key.rsplit_once(":graphics-").is_some_and(|(node_id, _)| {
                        document.nodes.iter().any(|node| node.node_id == node_id)
                    })
                });
            }
            let (rebuilds, entries) = indices.refresh(document, layout);
            if let Some(cache) = cache.as_mut() {
                cache.stats.index_rebuilds += rebuilds;
                cache.stats.index_entries_built += entries;
            }
        }
        let mut commands = Vec::new();
        let mut graphics_errors = Vec::new();
        let mut culled_draw_item_count = 0;
        let mut visible_slots = cache
            .as_mut()
            .map(|cache| std::mem::take(&mut cache.visible_slots))
            .unwrap_or_default();
        let mut changed_commands = incremental.then(Vec::new);
        if !incremental {
            visible_slots.clear();
        }
        let update_slots: Box<dyn Iterator<Item = usize>> = match changed_slots.as_ref() {
            Some(slots) => Box::new(slots.iter().copied()),
            None => Box::new(0..layout.computed_nodes.len()),
        };
        for computed_slot in update_slots {
            let computed = &layout.computed_nodes[computed_slot];
            if let Some(cache) = cache.as_mut() {
                cache.stats.nodes_visited += 1;
            }
            let old = retain.then(|| fragments[computed_slot].take()).flatten();
            let old_command_count = old.as_ref().map_or(0, |fragment| fragment.commands.len());
            visible_slots.remove(&computed_slot);
            if !computed.effective_visible {
                if let Some(cache) = cache.as_mut() {
                    let prefix = format!("{}:graphics-", computed.node_id);
                    cache
                        .graphics_errors
                        .retain(|key, _| !key.starts_with(&prefix));
                }
                if old_command_count > 0 {
                    changed_commands = None;
                }
                if incremental {
                    if let Some(cache) = cache.as_mut() {
                        cache.stats.nodes_compared += 1;
                    }
                }
                continue;
            }
            let Some(node) = indices
                .nodes
                .get(computed.node_id.as_str())
                .map(|slot| node_overrides.get(&slot).unwrap_or(&document.nodes[slot]))
            else {
                continue;
            };
            let visual_owner = overrides.and_then(|overrides| {
                aui_visual_override_owner(document, &indices, node.node_id.as_str(), overrides)
            });
            let visual = visual_owner.as_ref().map(|(_, visual)| **visual);
            let owner_rect = visual_owner.as_ref().and_then(|(owner_id, _)| {
                indices
                    .computed
                    .get(owner_id)
                    .map(|slot| layout.computed_nodes[slot].rect)
            });
            if let Some(cache) = cache.as_mut() {
                cache.stats.nodes_compared += 1;
                if let Some(old) = old.filter(|old| {
                    old.node == *node
                        && old.computed == *computed
                        && old.visual == visual
                        && old.owner_rect == owner_rect
                }) {
                    cache.stats.nodes_reused += 1;
                    cache.stats.command_payloads_reused += old.commands.len() as u64;
                    if retain {
                        visible_slots.insert(computed_slot);
                        fragments[computed_slot] = Some(old);
                    }
                    continue;
                }
                cache.stats.nodes_built += 1;
            }
            if let Some(cache) = cache.as_mut() {
                if !cache.graphics_errors.is_empty() {
                    let prefix = format!("{}:graphics-", computed.node_id);
                    cache
                        .graphics_errors
                        .retain(|key, _| !key.starts_with(&prefix));
                }
            }
            let mut fragment_commands = Vec::new();
            let fragment_culled_draw_item_count =
                if computed.clipped_by_node.is_some() && computed.effective_clip_rect.is_none() {
                    draw_command_count_for_node(node)
                } else {
                    0
                };
            let transform_rect = |rect: AuiComputedRect| match (visual, owner_rect) {
                _ if computed.presentation.is_some() => rect,
                (Some(visual), Some(owner_rect)) => {
                    aui_feedback_transform_rect(rect, owner_rect, visual)
                }
                _ => rect,
            };
            let transform_color = |color: Option<String>| match visual {
                _ if computed.presentation.is_some() => color,
                Some(visual) => aui_feedback_transform_color(color, visual),
                None => color,
            };
            if fragment_culled_draw_item_count == 0 {
                match node.kind {
                    AuiNodeKind::Panel
                    | AuiNodeKind::Button
                    | AuiNodeKind::InputField
                    | AuiNodeKind::Toggle
                    | AuiNodeKind::Slider
                    | AuiNodeKind::List
                    | AuiNodeKind::ScrollView => {
                        fragment_commands.push(Arc::new(AuiDrawCommand::DrawRect {
                            node_id: node.node_id.clone(),
                            rect: transform_rect(computed.rect),
                            effective_clip_rect: computed.effective_clip_rect,
                            color: transform_color(
                                node.style.as_ref().and_then(|style| style.color.clone()),
                            ),
                        }));
                        if matches!(node.kind, AuiNodeKind::Button | AuiNodeKind::InputField) {
                            let text = node.text.as_ref().or(node.placeholder.as_ref());
                            if let Some(text) = text {
                                fragment_commands.push(Arc::new(AuiDrawCommand::DrawText {
                                    node_id: format!("{}:label", node.node_id),
                                    rect: transform_rect(computed.rect),
                                    effective_clip_rect: computed.effective_clip_rect,
                                    text: text.clone(),
                                    color: transform_color(
                                        node.resolved_style
                                            .as_ref()
                                            .and_then(|style| style.paint.foreground.clone())
                                            .or_else(|| {
                                                node.style
                                                    .as_ref()
                                                    .and_then(|style| style.text_color.clone())
                                            }),
                                    ),
                                    font_size: node
                                        .style
                                        .as_ref()
                                        .and_then(|style| style.font_size),
                                    font: node.style.as_ref().and_then(|style| style.font.clone()),
                                }));
                            }
                        }
                    }
                    AuiNodeKind::ProgressBar => {
                        fragment_commands.push(Arc::new(AuiDrawCommand::DrawRect {
                            node_id: format!("{}:track", node.node_id),
                            rect: transform_rect(computed.rect),
                            effective_clip_rect: computed.effective_clip_rect,
                            color: transform_color(
                                node.style
                                    .as_ref()
                                    .and_then(|style| style.color.clone())
                                    .or_else(|| Some("#303030".to_string())),
                            ),
                        }));
                        let value = node.progress_value.unwrap_or(0.0).clamp(0.0, 1.0);
                        let fill_rect = AuiComputedRect {
                            x: computed.rect.x,
                            y: computed.rect.y,
                            width: computed.rect.width * value,
                            height: computed.rect.height,
                        };
                        fragment_commands.push(Arc::new(AuiDrawCommand::DrawRect {
                            node_id: format!("{}:fill", node.node_id),
                            rect: transform_rect(fill_rect),
                            effective_clip_rect: computed.effective_clip_rect,
                            color: transform_color(
                                node.resolved_style
                                    .as_ref()
                                    .and_then(|style| style.paint.foreground.clone())
                                    .or_else(|| {
                                        node.style
                                            .as_ref()
                                            .and_then(|style| style.text_color.clone())
                                    })
                                    .or_else(|| Some("#41d17d".to_string())),
                            ),
                        }));
                    }
                    AuiNodeKind::Image => {
                        if let Some(image) = &node.image {
                            fragment_commands.push(Arc::new(AuiDrawCommand::DrawImage {
                                node_id: node.node_id.clone(),
                                rect: transform_rect(computed.rect),
                                effective_clip_rect: computed.effective_clip_rect,
                                asset_id: image.asset_id.clone(),
                                color: transform_color(
                                    node.style.as_ref().and_then(|style| style.color.clone()),
                                ),
                            }));
                        }
                    }
                    AuiNodeKind::Text => {
                        if let Some(text) = &node.text {
                            fragment_commands.push(Arc::new(AuiDrawCommand::DrawText {
                                node_id: node.node_id.clone(),
                                rect: transform_rect(computed.rect),
                                effective_clip_rect: computed.effective_clip_rect,
                                text: text.clone(),
                                color: transform_color(
                                    node.resolved_style
                                        .as_ref()
                                        .and_then(|style| style.paint.foreground.clone())
                                        .or_else(|| {
                                            node.style
                                                .as_ref()
                                                .and_then(|style| style.text_color.clone())
                                        }),
                                ),
                                font_size: node.style.as_ref().and_then(|style| style.font_size),
                                font: node.style.as_ref().and_then(|style| style.font.clone()),
                            }));
                        }
                    }
                    AuiNodeKind::Custom => {
                        if let Some(paint) = node
                            .custom_runtime
                            .as_ref()
                            .and_then(|runtime| runtime.paint.as_ref())
                        {
                            if !paint.vertices.is_empty() && !paint.indices.is_empty() {
                                fragment_commands.push(Arc::new(AuiDrawCommand::DrawMesh {
                                    node_id: node.node_id.clone(),
                                    rect: transform_rect(computed.rect),
                                    effective_clip_rect: computed.effective_clip_rect,
                                    mesh: paint.clone().into(),
                                }));
                            }
                            for (primitive_index, primitive) in paint.primitives.iter().enumerate()
                            {
                                use project_runtime_sdk::aui_extension::UiExtensionPaintPrimitive;
                                let command = match primitive {
                                    UiExtensionPaintPrimitive::Graphics { .. }
                                    | UiExtensionPaintPrimitive::GraphicsAsset { .. } => {
                                        let id =
                                            format!("{}:graphics-{primitive_index}", node.node_id);
                                        let graphics = match primitive {
                                            UiExtensionPaintPrimitive::Graphics { graphics } => {
                                                Ok(graphics)
                                            }
                                            UiExtensionPaintPrimitive::GraphicsAsset {
                                                asset_id,
                                                guid,
                                            } => document
                                                .graphics_assets
                                                .resolve(asset_id, guid)
                                                .map(|asset| &asset.graphics),
                                            _ => unreachable!(),
                                        };
                                        if computed.presentation.and_then(|p| project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(p.local_to_canvas).ok()).is_some_and(|m| m.maximum_scale() == 0.0) { continue; }
                                        let result = graphics.and_then(|graphics| {
                                            let pixel_scale = cache
                                                .as_ref()
                                                .and_then(|cache| {
                                                    cache
                                                        .graphics_pixel_scales
                                                        .get(&computed.canvas_id)
                                                })
                                                .copied()
                                                .unwrap_or(1.0);
                                            let pixel_scale = computed.presentation.and_then(|p| project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(p.local_to_canvas).ok()).map_or(pixel_scale, |matrix| pixel_scale * matrix.maximum_scale() as f32);
                                            let graphics_cache = match cache.as_mut() {
                                                Some(cache) => &mut cache.graphics,
                                                None => &mut standalone_graphics,
                                            };
                                            graphics_cache.geometry(graphics, pixel_scale)
                                        });
                                        match result {
                                            Ok(geometry) if geometry.mesh.indices.is_empty() => {
                                                continue;
                                            }
                                            Ok(geometry) => AuiDrawCommand::DrawMesh {
                                                node_id: id,
                                                rect: transform_rect(computed.rect),
                                                effective_clip_rect: computed.effective_clip_rect,
                                                mesh: geometry.mesh.clone(),
                                            },
                                            Err(error) => {
                                                if let Some(cache) = cache.as_mut() {
                                                    cache
                                                        .graphics_errors
                                                        .insert(id.clone(), error.clone());
                                                }
                                                graphics_errors.push(format!("{id}: {error}"));
                                                continue;
                                            }
                                        }
                                    }
                                    UiExtensionPaintPrimitive::Rect { rect, color } => {
                                        let local = custom_primitive_rect(computed.rect, *rect);
                                        let command = AuiDrawCommand::DrawRect {
                                            node_id: format!(
                                                "{}:custom-rect-{primitive_index}",
                                                node.node_id
                                            ),
                                            rect: transform_rect(local),
                                            effective_clip_rect: computed.effective_clip_rect,
                                            color: Some(extension_color(*color)),
                                        };
                                        command
                                    }
                                    UiExtensionPaintPrimitive::Image {
                                        rect,
                                        asset_id,
                                        color,
                                    } => {
                                        let local = custom_primitive_rect(computed.rect, *rect);
                                        let command = AuiDrawCommand::DrawImage {
                                            node_id: format!(
                                                "{}:custom-image-{primitive_index}",
                                                node.node_id
                                            ),
                                            rect: transform_rect(local),
                                            effective_clip_rect: computed.effective_clip_rect,
                                            asset_id: asset_id.clone(),
                                            color: Some(extension_color(*color)),
                                        };
                                        command
                                    }
                                    UiExtensionPaintPrimitive::Text {
                                        rect,
                                        text,
                                        color,
                                        font_size,
                                    } => {
                                        let local = custom_primitive_rect(computed.rect, *rect);
                                        let command = AuiDrawCommand::DrawText {
                                            node_id: format!(
                                                "{}:custom-text-{primitive_index}",
                                                node.node_id
                                            ),
                                            rect: transform_rect(local),
                                            effective_clip_rect: computed.effective_clip_rect,
                                            text: text.clone(),
                                            color: Some(extension_color(*color)),
                                            font_size: Some(*font_size),
                                            font: None,
                                        };
                                        command
                                    }
                                };
                                fragment_commands.push(Arc::new(command));
                            }
                        }
                    }
                }
            }
            if let Some(cache) = cache.as_mut() {
                cache.stats.command_payloads_built += fragment_commands.len() as u64;
            }
            let fragment = drawing_cache::DrawFragment {
                node: node.clone(),
                computed: computed.clone(),
                visual,
                owner_rect,
                commands: fragment_commands,
                culled_draw_item_count: fragment_culled_draw_item_count,
            };
            if retain {
                visible_slots.insert(computed_slot);
                if old_command_count != fragment.commands.len() {
                    changed_commands = None;
                } else if let (Some(changed), Some(range)) = (
                    changed_commands.as_mut(),
                    cache
                        .as_ref()
                        .and_then(|cache| cache.command_ranges.get(&computed_slot)),
                ) {
                    changed.extend(range.clone());
                }
                fragments[computed_slot] = Some(fragment);
            } else {
                culled_draw_item_count += fragment.culled_draw_item_count;
                commands.extend(fragment.commands);
            }
        }
        if retain {
            let mut command_ranges = BTreeMap::new();
            for &slot in &visible_slots {
                let Some(fragment) = fragments[slot].as_ref() else {
                    continue;
                };
                culled_draw_item_count += fragment.culled_draw_item_count;
                let start = commands.len();
                commands.extend_from_slice(&fragment.commands);
                if changed_commands.is_none() {
                    command_ranges.insert(slot, start..commands.len());
                }
            }
            if let Some(cache) = cache.as_mut() {
                cache.stats.command_refs_submitted += commands.len() as u64;
                if incremental {
                    cache.stats.nodes_reused += (layout.computed_nodes.len()
                        - changed_slots.as_ref().map_or(0, Vec::len))
                        as u64;
                }
                if changed_commands.is_none() {
                    cache.command_ranges = command_ranges;
                }
            }
        }
        if let Some(cache) = cache.as_mut() {
            cache.nodes = fragments;
            cache.draw_node_overrides = node_overrides;
            cache.visible_slots = if retain {
                visible_slots
            } else {
                BTreeSet::new()
            };
            cache.pending_command_slots = if retain && layout.scrollbar_metrics.is_empty() {
                changed_commands
            } else {
                None
            };
            if !retain {
                cache.command_ranges.clear();
            }
            indices.trim();
            cache.indices = indices;
            if retain {
                cache.record_layout(layout);
            } else {
                cache.clear_layout();
            }
        }

        for metrics in layout
            .scrollbar_metrics
            .iter()
            .filter(|metrics| metrics.visible)
        {
            commands.push(Arc::new(AuiDrawCommand::DrawRect {
                node_id: metrics.track_node_id(),
                rect: metrics.track_rect,
                effective_clip_rect: None,
                color: Some("#1c232b".to_string()),
            }));
            commands.push(Arc::new(AuiDrawCommand::DrawRect {
                node_id: metrics.thumb_node_id(),
                rect: metrics.thumb_rect,
                effective_clip_rect: None,
                color: Some("#6f7d8c".to_string()),
            }));
            if let Some(cache) = cache.as_mut() {
                cache.stats.command_payloads_built += 2;
            }
        }

        let text_count = commands
            .iter()
            .filter(|command| matches!(command.as_ref(), AuiDrawCommand::DrawText { .. }))
            .count();
        let image_count = commands
            .iter()
            .filter(|command| matches!(command.as_ref(), AuiDrawCommand::DrawImage { .. }))
            .count();
        let effective_clip_item_count = commands
            .iter()
            .filter(|command| draw_command_effective_clip_rect(command).is_some())
            .count();
        let report = AuiRenderReport {
            draw_command_count: commands.len(),
            graphics_diagnostics: match cache.as_ref() {
                Some(cache) => cache
                    .graphics_errors
                    .iter()
                    .map(|(id, error)| format!("{id}: {error}"))
                    .collect(),
                None => graphics_errors,
            },
            text_count,
            image_count,
            effective_clip_item_count,
            culled_draw_item_count,
            scrollbar_visible_count: layout
                .scrollbar_metrics
                .iter()
                .filter(|metrics| metrics.visible)
                .count(),
            batch_hint_count: 0,
        };

        (AuiDrawList { commands }, report)
    }

    pub(super) fn layout_node(
        canvas: &AuiCanvas,
        nodes_by_id: &HashMap<&str, &AuiNode>,
        node_id: &str,
        parent_rect: AuiComputedRect,
        tree_order: &mut usize,
        computed_nodes: &mut Vec<AuiComputedNode>,
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
        active_clip: Option<AuiActiveClip>,
        scroll_offset_applied: &mut bool,
        scroll_applied_node_count: &mut usize,
        clipped_node_count: &mut usize,
        clip_root_count: &mut usize,
        parent_effective_visible: bool,
    ) {
        let Some(node) = nodes_by_id.get(node_id) else {
            return;
        };
        let rect = node.rect.resolve(parent_rect);
        let effective_clip_rect = active_clip
            .as_ref()
            .and_then(|clip| clip.rect.and_then(|clip_rect| rect.intersection(clip_rect)));
        let clipped_by_node = active_clip.as_ref().map(|clip| clip.node_id.clone());
        if active_clip.is_some() {
            *scroll_applied_node_count += 1;
            if effective_clip_rect.is_none() {
                *clipped_node_count += 1;
            }
        }
        let current_order = *tree_order;
        *tree_order += 1;
        let effective_visible = parent_effective_visible && node.visible;
        computed_nodes.push(AuiComputedNode {
            presentation: None,
            canvas_id: canvas.canvas_id.clone(),
            composition_stage: canvas.composition_stage,
            node_id: node.node_id.clone(),
            kind: node.kind,
            rect,
            effective_clip_rect,
            effective_clips: active_clip
                .as_ref()
                .map(|clip| clip.shapes.clone())
                .unwrap_or_default(),
            clipped_by_node: clipped_by_node.clone(),
            tree_order: current_order,
            local_visible: node.visible,
            effective_visible,
        });

        let offset_y = if matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List) {
            scroll_offsets
                .get(node.node_id.as_str())
                .map(|state| state.offset_y)
                .unwrap_or_default()
        } else {
            0.0
        };
        let child_parent_rect = if offset_y.abs() > f32::EPSILON {
            *scroll_offset_applied = true;
            AuiComputedRect {
                x: rect.x,
                y: rect.y - offset_y,
                width: rect.width,
                height: rect.height,
            }
        } else {
            rect
        };
        let node_is_clip_root = matches!(node.kind, AuiNodeKind::ScrollView | AuiNodeKind::List)
            || matches!(
                node.clip_policy,
                AuiClipPolicy::Rect | AuiClipPolicy::Rounded
            );
        if node_is_clip_root {
            *clip_root_count += 1;
        }
        let child_clip = if node_is_clip_root {
            let effective_rect = active_clip
                .as_ref()
                .and_then(|clip| clip.rect)
                .map_or(Some(rect), |parent_clip| rect.intersection(parent_clip));
            let mut shapes = active_clip
                .as_ref()
                .map(|clip| clip.shapes.clone())
                .unwrap_or_default();
            shapes.push(AuiResolvedClip {
                transform: None,
                node_id: node.node_id.clone(),
                rect,
                radius: if node.clip_policy == AuiClipPolicy::Rounded {
                    node.clip_radius.unwrap_or_default()
                } else {
                    0.0
                },
            });
            Some(AuiActiveClip {
                node_id: node.node_id.clone(),
                rect: effective_rect,
                shapes,
            })
        } else {
            active_clip
        };

        for child_id in &node.children {
            Self::layout_node(
                canvas,
                nodes_by_id,
                child_id,
                child_parent_rect,
                tree_order,
                computed_nodes,
                scroll_offsets,
                child_clip.clone(),
                scroll_offset_applied,
                scroll_applied_node_count,
                clipped_node_count,
                clip_root_count,
                effective_visible,
            );
        }
    }

    pub(super) fn compute_scrollbar_metrics(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        computed_nodes: &[AuiComputedNode],
        scroll_offsets: &BTreeMap<String, AuiScrollState>,
    ) -> Vec<AuiScrollbarMetrics> {
        let computed_by_id = computed_nodes
            .iter()
            .map(|computed| (computed.node_id.as_str(), computed))
            .collect::<HashMap<_, _>>();
        let mut metrics = Vec::new();
        for computed in computed_nodes
            .iter()
            .filter(|computed| matches!(computed.kind, AuiNodeKind::ScrollView | AuiNodeKind::List))
        {
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                continue;
            };
            if node.scrollbar_policy == AuiScrollbarPolicy::None {
                continue;
            }
            let viewport = computed.rect;
            let mut max_bottom = viewport.y + viewport.height;
            for child in computed_nodes {
                if child.node_id == computed.node_id {
                    continue;
                }
                if Self::layout_node_in_subtree(
                    nodes_by_id,
                    computed.node_id.as_str(),
                    child.node_id.as_str(),
                ) {
                    max_bottom = max_bottom.max(child.rect.y + child.rect.height);
                }
            }
            let content_height = (max_bottom - viewport.y).max(viewport.height);
            let max_offset_y = (content_height - viewport.height).max(0.0);
            let offset_y = scroll_offsets
                .get(computed.node_id.as_str())
                .map(|state| state.offset_y.clamp(0.0, max_offset_y))
                .unwrap_or_default();
            let visible = computed.effective_visible
                && match node.scrollbar_policy {
                    AuiScrollbarPolicy::None => false,
                    AuiScrollbarPolicy::Auto => max_offset_y > f32::EPSILON,
                    AuiScrollbarPolicy::Always => true,
                };
            let track_width = 8.0_f32.min(viewport.width.max(0.0));
            let track_rect = AuiComputedRect {
                x: viewport.x + viewport.width - track_width,
                y: viewport.y,
                width: track_width,
                height: viewport.height,
            };
            let thumb_height = if content_height <= f32::EPSILON {
                viewport.height
            } else {
                (viewport.height * (viewport.height / content_height))
                    .clamp(18.0_f32.min(viewport.height), viewport.height)
            };
            let travel = (viewport.height - thumb_height).max(0.0);
            let thumb_y = if max_offset_y <= f32::EPSILON {
                viewport.y
            } else {
                viewport.y + travel * (offset_y / max_offset_y)
            };
            let thumb_rect = AuiComputedRect {
                x: track_rect.x,
                y: thumb_y,
                width: track_rect.width,
                height: thumb_height,
            };
            if computed_by_id.contains_key(computed.node_id.as_str()) {
                metrics.push(AuiScrollbarMetrics {
                    scroll_node_id: computed.node_id.clone(),
                    axis: AuiScrollbarAxis::Vertical,
                    track_rect,
                    thumb_rect,
                    offset_y,
                    max_offset_y,
                    viewport_height: viewport.height,
                    content_height,
                    visible,
                });
            }
        }
        metrics
    }

    pub(super) fn layout_node_in_subtree(
        nodes_by_id: &HashMap<&str, &AuiNode>,
        root_node: &str,
        node_id: &str,
    ) -> bool {
        if root_node == node_id {
            return true;
        }
        let mut current = node_id;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some(node) = nodes_by_id.get(current) else {
                return false;
            };
            let Some(parent) = node.parent.as_deref() else {
                return false;
            };
            if parent == root_node {
                return true;
            }
            current = parent;
        }
        false
    }

    pub(super) fn is_full_screen_single_image_ui(document: &AuiDocument) -> bool {
        if document.canvases.len() != 1 || document.nodes.len() != 1 {
            return false;
        }
        let canvas = &document.canvases[0];
        let node = &document.nodes[0];
        node.kind == AuiNodeKind::Image
            && node.node_id == canvas.root_node
            && node
                .rect
                .resolve(AuiComputedRect {
                    x: 0.0,
                    y: 0.0,
                    width: canvas.reference_resolution.x,
                    height: canvas.reference_resolution.y,
                })
                .approximately_full_screen(canvas.reference_resolution)
    }
}
