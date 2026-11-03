use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiProjectedFontBatch {
    pub render_mode: FontBundleRenderMode,
    pub page_index: u32,
    pub glyph_count: usize,
    pub texture: RenderResourceHandle,
    pub font_render_mode: Option<FontBundleRenderMode>,
    pub vertices: Vec<RenderDrawVertex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UiProjectedBatchKey {
    Solid,
    Retained(u64, [u32; 4]),
    Texture(RenderResourceHandle),
    Font(RenderResourceHandle, FontBundleRenderMode, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct UiProjectedOrderedBatch {
    pub(super) key: UiProjectedBatchKey,
    pub(super) texture: Option<RenderResourceHandle>,
    pub(super) font_render_mode: Option<FontBundleRenderMode>,
    pub(super) font_page_index: Option<u32>,
    pub(super) glyph_count: usize,
    pub(super) debug_label: String,
    pub(super) vertices: UiVertexStream,
    pub(super) retained: Vec<crate::aui_retained::UiRetainedDraw>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct UiProjectedOrderedBatches {
    pub(super) batches: Vec<UiProjectedOrderedBatch>,
    pub(super) diagnostics: Vec<RenderGraphDiagnostic>,
}

/// Cumulative work counters, read on demand; no per-frame serialization/logging.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiProjectionStats {
    pub cpu_ns: u64,
    pub items_compared: u64,
    pub fragments_reused: u64,
    pub fragments_merged: u64,
    pub vertices_copied: u64,
    pub stream_chunks_reused: u64,
    pub stream_chunks_changed: u64,
    pub mapping_items_scanned: u64,
    pub stage_hits: u64,
    pub stage_rebuilds: u64,
    pub fallback_stage_scans: u64,
    pub hint_slots_consumed: u64,
    pub items_projected: u64,
    pub glyphs_indexed: u64,
    pub glyph_groups_updated: u64,
    pub vertices_generated: u64,
    pub batches_reused: u64,
}

impl UiProjectionStats {
    pub fn since(self, previous: Self) -> Self {
        macro_rules! delta {
            ($($field:ident),* $(,)?) => { Self { $($field: self.$field.saturating_sub(previous.$field)),* } };
        }
        delta!(
            cpu_ns,
            items_compared,
            fragments_reused,
            fragments_merged,
            vertices_copied,
            stream_chunks_reused,
            stream_chunks_changed,
            mapping_items_scanned,
            stage_hits,
            stage_rebuilds,
            fallback_stage_scans,
            hint_slots_consumed,
            items_projected,
            glyphs_indexed,
            glyph_groups_updated,
            vertices_generated,
            batches_reused
        )
    }
}

#[derive(Debug, Default)]
pub(super) struct UiProjectionCache {
    pub(super) measurement_enabled: bool,
    pub(super) force_cpu_graphics: bool,
    pub(super) stages: BTreeMap<AuiCompositionStage, CachedUiStage>,
    pub(super) stage_variants: BTreeMap<AuiCompositionStage, CachedUiStage>,
    pub(super) fallback_presentations: BTreeMap<AuiCompositionStage, CachedUiPresentation>,
    pub(super) stats: UiProjectionStats,
}

#[derive(Debug)]
pub(super) struct CachedUiPresentation {
    pub(super) items: Arc<Vec<Arc<crate::aui::AuiOverlayDrawItem>>>,
    pub(super) references: Vec<CanvasReferenceFact>,
    pub(super) extent: [f32; 2],
    pub(super) scale_policy: GameViewScalePolicy,
    pub(super) presentation: ResolvedGameViewPresentation,
}

#[derive(Debug, Clone)]
pub(super) struct CachedUiStage {
    pub(super) items: Arc<Vec<Arc<crate::aui::AuiOverlayDrawItem>>>,
    pub(super) glyphs: Option<Arc<crate::aui::AuiTextGlyphPlan>>,
    pub(super) presentation: ResolvedGameViewPresentation,
    pub(super) textures: Option<RuntimeTextureBindingContext>,
    pub(super) projected: UiProjectedOrderedBatches,
    pub(super) fragments: Vec<UiProjectedOrderedBatches>,
    pub(super) glyph_index: BTreeMap<String, Arc<Vec<Arc<crate::aui::AuiTextGlyphQuad>>>>,
    pub(super) item_slots: BTreeMap<String, usize>,
}

impl UiProjectionCache {
    pub(super) fn stage_matches(
        cached: &CachedUiStage,
        composition: &AuiCompositionFrame,
        stage: &AuiCompositionStageFrame,
        presentation: &ResolvedGameViewPresentation,
        textures: Option<&RuntimeTextureBindingContext>,
    ) -> bool {
        Arc::ptr_eq(&cached.items, &stage.draw_items)
            && match (&cached.glyphs, &composition.glyph_plan) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && cached.presentation.same_ui_mapping(presentation)
            && match (&cached.textures, textures) {
                (Some(a), Some(b)) => a.shares_storage(b) || a == b,
                (None, None) => true,
                _ => false,
            }
    }

    pub(super) fn resolve_presentation(
        &mut self,
        composition: &AuiCompositionFrame,
        stage: &AuiCompositionStageFrame,
        extent: [f32; 2],
        target_id: &str,
        scale_policy: GameViewScalePolicy,
    ) -> Result<ResolvedGameViewPresentation, &'static str> {
        if let Some(previous) = self.fallback_presentations.get(&stage.stage).filter(|p| {
            Arc::ptr_eq(&p.items, &stage.draw_items)
                && p.references == composition.canvas_references
                && p.extent == extent
                && p.scale_policy == scale_policy
        }) {
            return Ok(previous.presentation.clone());
        }
        self.stats.mapping_items_scanned += stage.draw_items.len() as u64;
        let presentation =
            resolve_ui_presentation(composition, stage, extent, target_id, scale_policy)?;
        self.fallback_presentations.insert(
            stage.stage,
            CachedUiPresentation {
                items: stage.draw_items.clone(),
                references: composition.canvas_references.clone(),
                extent,
                scale_policy,
                presentation: presentation.clone(),
            },
        );
        Ok(presentation)
    }

    pub(super) fn project(
        &mut self,
        composition: &AuiCompositionFrame,
        stage: &AuiCompositionStageFrame,
        presentation: &ResolvedGameViewPresentation,
        textures: Option<&RuntimeTextureBindingContext>,
    ) -> UiProjectedOrderedBatches {
        let started = self
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let projected = self.project_inner(composition, stage, presentation, textures);
        self.stats.cpu_ns += crate::aui::aui_elapsed_ns(started);
        projected
    }

    pub(super) fn project_inner(
        &mut self,
        composition: &AuiCompositionFrame,
        stage: &AuiCompositionStageFrame,
        presentation: &ResolvedGameViewPresentation,
        textures: Option<&RuntimeTextureBindingContext>,
    ) -> UiProjectedOrderedBatches {
        if self.stages.get(&stage.stage).is_some_and(|cached| {
            Self::stage_matches(cached, composition, stage, presentation, textures)
        }) {
            self.stats.stage_hits += 1;
            return self.stages[&stage.stage].projected.clone();
        }
        if self.stage_variants.get(&stage.stage).is_some_and(|cached| {
            Self::stage_matches(cached, composition, stage, presentation, textures)
        }) {
            let variant = self
                .stage_variants
                .remove(&stage.stage)
                .expect("matched bounded UI stage variant");
            if let Some(active) = self.stages.insert(stage.stage, variant) {
                self.stage_variants.insert(stage.stage, active);
            }
            self.stats.stage_hits += 1;
            return self.stages[&stage.stage].projected.clone();
        }
        let mut previous = self.stages.remove(&stage.stage);
        let backup_candidate = previous.as_ref().cloned();
        self.stage_variants.remove(&stage.stage);
        let same_mapping = previous.as_ref().is_some_and(|p| {
            p.presentation.same_ui_mapping(presentation)
                && match (&p.textures, textures) {
                    (Some(a), Some(b)) => a.shares_storage(b) || a == b,
                    (None, None) => true,
                    _ => false,
                }
        });
        let same_glyph_plan =
            previous
                .as_ref()
                .is_some_and(|p| match (&p.glyphs, &composition.glyph_plan) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                });
        let stage_hint = previous.as_ref().and_then(|old| {
            composition
                .projection_delta()?
                .stages
                .get(&stage.stage)
                .filter(|hint| {
                    same_mapping
                        && Arc::ptr_eq(&hint.previous_items, &old.items)
                        && Arc::ptr_eq(&hint.current_items, &stage.draw_items)
                        && hint
                            .changed_slots
                            .iter()
                            .all(|slot| *slot < stage.draw_items.len())
                })
                .cloned()
        });
        let glyph_hint = previous.as_ref().and_then(|old| {
            let hint = composition.projection_delta()?.glyphs.as_ref()?;
            match (&old.glyphs, &composition.glyph_plan) {
                (Some(previous_plan), Some(current_plan))
                    if Arc::ptr_eq(previous_plan, &hint.previous_plan)
                        && Arc::ptr_eq(current_plan, &hint.current_plan) =>
                {
                    Some(hint.clone())
                }
                _ => None,
            }
        });
        let glyph_index = if same_glyph_plan {
            std::mem::take(&mut previous.as_mut().unwrap().glyph_index)
        } else if let Some(hint) = glyph_hint.as_ref() {
            let mut index = std::mem::take(&mut previous.as_mut().unwrap().glyph_index);
            for (item_id, quads) in hint.changed_groups.iter() {
                self.stats.glyph_groups_updated += 1;
                if quads.is_empty() {
                    index.remove(item_id);
                } else {
                    index.insert(item_id.clone(), quads.clone());
                }
            }
            index
        } else {
            self.stats.glyphs_indexed +=
                composition.glyph_plan.as_ref().map_or(0, |p| p.quads.len()) as u64;
            ui_glyph_index(composition)
        };
        let (fragments, item_slots) =
            if let (Some(hint), Some(old)) = (stage_hint.as_ref(), previous.as_mut()) {
                let mut changed_slots = hint.changed_slots.as_ref().clone();
                if let Some(glyph_hint) = glyph_hint.as_ref() {
                    changed_slots.extend(
                        glyph_hint
                            .changed_groups
                            .keys()
                            .filter_map(|item_id| old.item_slots.get(item_id).copied()),
                    );
                }
                changed_slots.sort_unstable();
                changed_slots.dedup();
                self.stats.hint_slots_consumed += changed_slots.len() as u64;
                self.stats.items_compared += changed_slots.len() as u64;
                self.stats.fragments_reused +=
                    stage.draw_items.len().saturating_sub(changed_slots.len()) as u64;
                let mut fragments = std::mem::take(&mut old.fragments);
                let mut item_slots = std::mem::take(&mut old.item_slots);
                for slot in changed_slots {
                    if let Some(previous_item) = old.items.get(slot) {
                        item_slots.remove(previous_item.item_id.as_str());
                    }
                    let item = &stage.draw_items[slot];
                    item_slots.insert(item.item_id.clone(), slot);
                    self.stats.items_projected += 1;
                    let fragment = ui_projection_items(
                        composition,
                        std::slice::from_ref(item),
                        presentation,
                        textures,
                        &glyph_index,
                        self.force_cpu_graphics,
                    );
                    self.stats.vertices_generated += fragment
                        .batches
                        .iter()
                        .map(|b| b.vertices.len() as u64)
                        .sum::<u64>();
                    fragments[slot] = fragment;
                }
                (fragments, item_slots)
            } else {
                if previous.is_some() {
                    self.stats.fallback_stage_scans += 1;
                }
                let mut fragments = Vec::with_capacity(stage.draw_items.len());
                let mut item_slots = BTreeMap::new();
                for (slot, item) in stage.draw_items.iter().enumerate() {
                    self.stats.items_compared += 1;
                    item_slots.insert(item.item_id.clone(), slot);
                    let reusable = same_mapping
                        && previous.as_ref().is_some_and(|p| {
                            p.items
                                .get(slot)
                                .is_some_and(|old| Arc::ptr_eq(old, item) || old == item)
                                && (item.item_kind != AuiOverlayItemKind::Text
                                    || same_glyph_plan
                                    || ui_item_glyphs_equal(
                                        p,
                                        composition,
                                        &glyph_index,
                                        &item.item_id,
                                    ))
                        });
                    let fragment = if reusable {
                        self.stats.fragments_reused += 1;
                        previous.as_ref().unwrap().fragments[slot].clone()
                    } else {
                        self.stats.items_projected += 1;
                        let fragment = ui_projection_items(
                            composition,
                            std::slice::from_ref(item),
                            presentation,
                            textures,
                            &glyph_index,
                            self.force_cpu_graphics,
                        );
                        self.stats.vertices_generated += fragment
                            .batches
                            .iter()
                            .map(|b| b.vertices.len() as u64)
                            .sum::<u64>();
                        fragment
                    };
                    fragments.push(fragment);
                }
                (fragments, item_slots)
            };
        let mut projected = UiProjectedOrderedBatches::default();
        for fragment in &fragments {
            self.stats.fragments_merged += 1;
            append_ui_fragment(&mut projected, fragment, &mut self.stats);
        }
        self.stats.stage_rebuilds += 1;
        // Preserve stable stream lineage without flattening immutable item chunks.
        if let Some(previous) = previous.as_ref() {
            for (next, old) in projected
                .batches
                .iter_mut()
                .zip(&previous.projected.batches)
            {
                if next.key == old.key {
                    let (reused, changed) = next.vertices.continue_from(&old.vertices);
                    self.stats.stream_chunks_reused += reused as u64;
                    self.stats.stream_chunks_changed += changed as u64;
                    if changed == 0 && ui_batch_metadata_equal(next, old) {
                        next.vertices = old.vertices.clone();
                        self.stats.batches_reused += 1;
                    }
                }
            }
        }
        self.stages.remove(&stage.stage);
        let vertex_bytes = |p: &UiProjectedOrderedBatches| {
            p.batches
                .iter()
                .map(|b| {
                    b.vertices.allocated_vertex_capacity() * std::mem::size_of::<RenderDrawVertex>()
                })
                .sum::<usize>()
        };
        let bytes = vertex_bytes(&projected) + fragments.iter().map(&vertex_bytes).sum::<usize>();
        const MAX_VERTEX_BYTES: usize = 32 * 1024 * 1024;
        if bytes <= MAX_VERTEX_BYTES
            && stage.draw_items.len() <= 16_384
            && composition.glyph_plan.as_ref().map_or(0, |p| p.quads.len()) <= 65_536
        {
            if self
                .stages
                .values()
                .map(|s| {
                    vertex_bytes(&s.projected)
                        + s.fragments.iter().map(&vertex_bytes).sum::<usize>()
                })
                .sum::<usize>()
                + bytes
                > MAX_VERTEX_BYTES
            {
                self.stages.clear();
            }
            self.stages.insert(
                stage.stage,
                CachedUiStage {
                    items: stage.draw_items.clone(),
                    glyphs: composition.glyph_plan.clone(),
                    presentation: presentation.clone(),
                    textures: textures.cloned(),
                    projected: projected.clone(),
                    fragments,
                    glyph_index,
                    item_slots,
                },
            );
            if let Some(backup) = backup_candidate.filter(bounded_ui_stage_variant) {
                self.stage_variants.insert(stage.stage, backup);
            }
        }
        projected
    }
}

pub(super) fn ui_batch_metadata_equal(
    next: &UiProjectedOrderedBatch,
    old: &UiProjectedOrderedBatch,
) -> bool {
    next.retained == old.retained
        && next.key == old.key
        && next.texture == old.texture
        && next.font_render_mode == old.font_render_mode
        && next.font_page_index == old.font_page_index
        && next.glyph_count == old.glyph_count
        && next.debug_label == old.debug_label
}

pub(super) fn bounded_ui_stage_variant(stage: &CachedUiStage) -> bool {
    const MAX_VARIANT_ITEMS: usize = 4_096;
    const MAX_VARIANT_VERTEX_BYTES: usize = 8 * 1024 * 1024;
    if stage.items.len() > MAX_VARIANT_ITEMS {
        return false;
    }
    let vertex_bytes = |projected: &UiProjectedOrderedBatches| {
        projected
            .batches
            .iter()
            .map(|batch| {
                batch.vertices.allocated_vertex_capacity() * std::mem::size_of::<RenderDrawVertex>()
            })
            .sum::<usize>()
    };
    vertex_bytes(&stage.projected) + stage.fragments.iter().map(vertex_bytes).sum::<usize>()
        <= MAX_VARIANT_VERTEX_BYTES
}

pub(super) fn ui_item_glyphs_equal(
    previous: &CachedUiStage,
    composition: &AuiCompositionFrame,
    index: &BTreeMap<String, Arc<Vec<Arc<crate::aui::AuiTextGlyphQuad>>>>,
    item_id: &str,
) -> bool {
    match (&previous.glyphs, &composition.glyph_plan) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let old = previous
                .glyph_index
                .get(item_id)
                .map(|quads| quads.as_slice())
                .unwrap_or_default();
            let next = index
                .get(item_id)
                .map(|quads| quads.as_slice())
                .unwrap_or_default();
            a.font_source_kind == b.font_source_kind
                && a.font_atlas_id == b.font_atlas_id
                && a.atlas_generation == b.atlas_generation
                && old.len() == next.len()
                && old
                    .iter()
                    .zip(next)
                    .all(|(old, next)| Arc::ptr_eq(old, next) || old == next)
        }
        _ => false,
    }
}

pub(super) fn append_ui_fragment(
    output: &mut UiProjectedOrderedBatches,
    fragment: &UiProjectedOrderedBatches,
    _stats: &mut UiProjectionStats,
) {
    output
        .diagnostics
        .extend(fragment.diagnostics.iter().cloned());
    for batch in &fragment.batches {
        if let Some(last) = output
            .batches
            .last_mut()
            .filter(|last| last.key == batch.key)
        {
            last.glyph_count += batch.glyph_count;
            last.vertices.extend_chunks_from(&batch.vertices);
            last.retained.extend(batch.retained.iter().cloned());
        } else {
            output.batches.push(batch.clone());
        }
    }
}

#[cfg(test)]
pub(super) fn ui_projection_ordered_batches(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
    texture_bindings: Option<&RuntimeTextureBindingContext>,
) -> UiProjectedOrderedBatches {
    ui_projection_items(
        composition,
        &stage.draw_items,
        presentation,
        texture_bindings,
        &ui_glyph_index(composition),
        false,
    )
}

pub(super) fn ui_glyph_index(
    composition: &AuiCompositionFrame,
) -> BTreeMap<String, Arc<Vec<Arc<crate::aui::AuiTextGlyphQuad>>>> {
    let mut glyphs_by_item = BTreeMap::<String, Vec<Arc<crate::aui::AuiTextGlyphQuad>>>::new();
    if let Some(plan) = composition.glyph_plan.as_ref() {
        for glyph in &plan.quads {
            glyphs_by_item
                .entry(glyph.item_id.clone())
                .or_default()
                .push(glyph.clone());
        }
    }
    glyphs_by_item
        .into_iter()
        .map(|(item_id, quads)| (item_id, Arc::new(quads)))
        .collect()
}

pub(super) fn ui_projection_items(
    composition: &AuiCompositionFrame,
    items: &[Arc<crate::aui::AuiOverlayDrawItem>],
    presentation: &ResolvedGameViewPresentation,
    texture_bindings: Option<&RuntimeTextureBindingContext>,
    glyphs_by_item: &BTreeMap<String, Arc<Vec<Arc<crate::aui::AuiTextGlyphQuad>>>>,
    force_cpu_graphics: bool,
) -> UiProjectedOrderedBatches {
    let mut output = UiProjectedOrderedBatches::default();
    for item in items {
        let item = item.as_ref();
        let canvas_scale = canvas_pixel_scale(presentation, &item.canvas_id);
        let clip_budget = item.effective_clips.len() <= 8
            && resolved_item_clips(item).iter().all(|clip| {
                let scale = clip
                    .transform
                    .and_then(|m| {
                        project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(m).ok()
                    })
                    .map_or(1., |m| m.maximum_scale() as f32)
                    * canvas_scale;
                rounded_rect_segments(clip.rect, clip.radius, scale).is_some()
            });
        let paint_budget = item.paint.as_ref().is_none_or(|paint| {
            let scale = item_pixel_scale(presentation, item);
            rounded_rect_segments(item.rect, paint.radius, scale).is_some()
                && (!paint.shadow.visible()
                    || rounded_rect_segments(
                        inset_rect(item.rect, -(paint.shadow.spread + paint.shadow.blur)),
                        paint.radius + paint.shadow.spread + paint.shadow.blur,
                        scale,
                    )
                    .is_some())
        });
        if !clip_budget || !paint_budget {
            output.diagnostics.push(RenderGraphDiagnostic::error("aui_presentation.geometry_budget",format!("Node '{}' exceeds 8 ancestor clips or 256 segments per rounded corner at 0.25 pixel precision.",item.node_id)));
            continue;
        }
        match item.item_kind {
            AuiOverlayItemKind::Rect
            | AuiOverlayItemKind::ScrollbarTrack
            | AuiOverlayItemKind::ScrollbarThumb => {
                if let Some(paint) = item.paint.as_ref() {
                    append_resolved_paint_batches(
                        &mut output,
                        item,
                        paint,
                        presentation,
                        texture_bindings,
                    );
                    continue;
                }
                let Some(color) = parse_aui_color(item.color.as_deref()) else {
                    continue;
                };
                let vertices = projected_clipped_quad_vertices(
                    presentation,
                    item.canvas_id.as_str(),
                    item.rect,
                    item,
                    color,
                    [0.0, 0.0, 1.0, 1.0],
                );
                if vertices.is_empty() {
                    continue;
                }
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Solid,
                    None,
                    None,
                    None,
                    0,
                    "solid".to_string(),
                    vertices,
                );
            }
            AuiOverlayItemKind::Image => {
                let Some(asset_id) = item.asset_id.as_deref() else {
                    output.diagnostics.push(RenderGraphDiagnostic::error(
                        "aui_image.asset_id_missing",
                        format!("AUI Image item '{}' has no asset_id.", item.item_id),
                    ));
                    continue;
                };
                let Some(binding) = texture_bindings.and_then(|bindings| bindings.get(asset_id))
                else {
                    output.diagnostics.push(RenderGraphDiagnostic::error(
                        "aui_image.texture_not_resolved",
                        format!(
                            "AUI Image item '{}' could not resolve RuntimePackage texture asset '{}'.",
                            item.item_id, asset_id
                        ),
                    ));
                    continue;
                };
                let Some(vertices) = ui_projection_image_vertices(item, presentation) else {
                    continue;
                };
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Texture(binding.handle),
                    Some(binding.handle),
                    None,
                    None,
                    0,
                    format!("image:{asset_id}"),
                    vertices,
                );
            }
            AuiOverlayItemKind::Text => {
                let Some(glyphs) = glyphs_by_item.get(item.item_id.as_str()) else {
                    continue;
                };
                let Some(plan) = composition.glyph_plan.as_ref() else {
                    continue;
                };
                for glyph in glyphs.iter() {
                    let color = parse_aui_color(glyph.color.as_deref().or(item.color.as_deref()))
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let formal_v2 = uses_font_bundle_pages(&plan.font_source_kind);
                    let handle = if formal_v2 {
                        font_bundle_page_generation_render_handle(
                            glyph
                                .font_bundle
                                .as_ref()
                                .map(|b| b.0.as_str())
                                .unwrap_or(&plan.font_atlas_id),
                            glyph.render_mode,
                            glyph.page_index,
                            glyph
                                .font_bundle
                                .as_ref()
                                .map(|b| b.1)
                                .unwrap_or(plan.atlas_generation),
                        )
                    } else {
                        font_atlas_render_handle(&plan.font_atlas_id)
                    };
                    let [u0, v0, u1, v1] = glyph.uv_rect;
                    let vertices = projected_clipped_quad_vertices(
                        presentation,
                        item.canvas_id.as_str(),
                        glyph.rect,
                        item,
                        color,
                        [u0, v0, u1, v1],
                    );
                    if vertices.is_empty() {
                        continue;
                    }
                    append_ui_projected_batch(
                        &mut output.batches,
                        UiProjectedBatchKey::Font(handle, glyph.render_mode, glyph.page_index),
                        Some(handle),
                        formal_v2.then_some(glyph.render_mode),
                        Some(glyph.page_index),
                        1,
                        format!("font:{:?}:{}", glyph.render_mode, glyph.page_index),
                        vertices,
                    );
                }
            }
            AuiOverlayItemKind::Mesh => {
                let Some(mesh) = item.mesh.as_ref() else {
                    continue;
                };
                if let Some(draw) = (!force_cpu_graphics)
                    .then(|| retained_graphics_draw(item, mesh, presentation))
                    .flatten()
                {
                    let key = UiProjectedBatchKey::Retained(draw.geometry.id, draw.scissor);
                    if let Some(last) = output.batches.last_mut().filter(|b| b.key == key) {
                        last.retained.push(draw);
                    } else {
                        output.batches.push(UiProjectedOrderedBatch {
                            key,
                            texture: None,
                            font_render_mode: None,
                            font_page_index: None,
                            glyph_count: 0,
                            debug_label: "graphics:retained".into(),
                            vertices: Default::default(),
                            retained: vec![draw],
                        });
                    }
                    continue;
                }
                let vertices = projected_custom_mesh_vertices(item, mesh, presentation);
                if vertices.is_empty() {
                    continue;
                }
                let (key, texture, label) = if let Some(asset_id) = mesh.asset_id.as_deref() {
                    let Some(binding) =
                        texture_bindings.and_then(|bindings| bindings.get(asset_id))
                    else {
                        output.diagnostics.push(RenderGraphDiagnostic::error(
                            "aui_mesh.texture_not_resolved",
                            format!(
                                "AUI mesh item '{}' could not resolve texture asset '{}'.",
                                item.item_id, asset_id
                            ),
                        ));
                        continue;
                    };
                    (
                        UiProjectedBatchKey::Texture(binding.handle),
                        Some(binding.handle),
                        format!("mesh:{asset_id}"),
                    )
                } else {
                    (UiProjectedBatchKey::Solid, None, "mesh:solid".to_string())
                };
                append_ui_projected_batch(
                    &mut output.batches,
                    key,
                    texture,
                    None,
                    None,
                    0,
                    label,
                    vertices,
                );
            }
        }
    }
    output
}

pub(super) fn retained_graphics_draw(
    item: &crate::aui::AuiOverlayDrawItem,
    mesh: &crate::aui_retained::AuiMeshPayload,
    presentation: &ResolvedGameViewPresentation,
) -> Option<crate::aui_retained::UiRetainedDraw> {
    let geometry = mesh.local.as_ref()?.clone();
    if mesh.asset_id.is_some() {
        return None;
    }
    let extent = presentation.canvas_reference_extent(&item.canvas_id)?;
    let mut bounds = [0., 0., extent.width as f32, extent.height as f32];
    for clip in resolved_item_clips(item) {
        if clip.radius != 0. {
            return None;
        }
        let m = clip.transform.unwrap_or([1., 0., 0., 1., 0., 0.]);
        if m[1] != 0. || m[2] != 0. {
            return None;
        }
        let x0 = m[0] * clip.rect.x + m[4];
        let x1 = m[0] * (clip.rect.x + clip.rect.width) + m[4];
        let y0 = m[3] * clip.rect.y + m[5];
        let y1 = m[3] * (clip.rect.y + clip.rect.height) + m[5];
        bounds[0] = bounds[0].max(x0.min(x1));
        bounds[1] = bounds[1].max(y0.min(y1));
        bounds[2] = bounds[2].min(x0.max(x1));
        bounds[3] = bounds[3].min(y0.max(y1));
    }
    if bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
        return None;
    }
    let mapped = presentation
        .reference_rect_to_target(
            &item.canvas_id,
            GameViewRect::new(
                bounds[0],
                bounds[1],
                bounds[2] - bounds[0],
                bounds[3] - bounds[1],
            ),
        )
        .ok()?;
    let w = presentation.target_extent.width as f32;
    let h = presentation.target_extent.height as f32;
    let edges = [
        mapped.x.max(0.),
        mapped.y.max(0.),
        (mapped.x + mapped.width).min(w),
        (mapped.y + mapped.height).min(h),
    ];
    // Fractional scissor boundaries could change coverage; preserve precise CPU semantics.
    if edges
        .iter()
        .any(|v| !v.is_finite() || (*v - v.round()).abs() > 0.0001)
    {
        return None;
    }
    let edges = edges.map(|v| v.round().max(0.) as u32);
    if edges[2] <= edges[0] || edges[3] <= edges[1] {
        return None;
    }
    let origin = presentation
        .reference_rect_to_target(&item.canvas_id, GameViewRect::new(0., 0., 1., 1.))
        .ok()?;
    let m = item
        .presentation
        .map(|p| p.vertex_transform)
        .unwrap_or([1., 0., 0., 1., 0., 0.]);
    let tint = item.presentation.map(|p| p.modulation).unwrap_or([1.; 4]);
    let values = [
        m[0],
        m[2],
        m[0] * item.rect.x + m[2] * item.rect.y + m[4],
        0.,
        m[1],
        m[3],
        m[1] * item.rect.x + m[3] * item.rect.y + m[5],
        0.,
        tint[0],
        tint[1],
        tint[2],
        tint[3],
        2. * origin.width / w,
        -2. * origin.height / h,
        2. * origin.x / w - 1.,
        1. - 2. * origin.y / h,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(crate::aui_retained::UiRetainedDraw {
        geometry,
        parameters: values.map(Into::into),
        scissor: [edges[0], edges[1], edges[2] - edges[0], edges[3] - edges[1]],
        target_extent: [
            presentation.target_extent.width,
            presentation.target_extent.height,
        ],
    })
}

pub(super) fn projected_custom_mesh_vertices(
    item: &crate::aui::AuiOverlayDrawItem,
    mesh: &project_runtime_sdk::aui_extension::UiExtensionPaintOutput,
    presentation: &ResolvedGameViewPresentation,
) -> Vec<RenderDrawVertex> {
    let clips = resolved_item_clips(item);
    if clips.is_empty() && item.presentation.is_none() {
        let width = presentation.target_extent.width.max(1) as f32;
        let height = presentation.target_extent.height.max(1) as f32;
        return mesh
            .indices
            .iter()
            .filter_map(|&index| mesh.vertices.get(index as usize))
            .filter_map(|vertex| {
                let target = presentation
                    .reference_rect_to_target(
                        item.canvas_id.as_str(),
                        GameViewRect::new(
                            item.rect.x + vertex.position[0],
                            item.rect.y + vertex.position[1],
                            0.0,
                            0.0,
                        ),
                    )
                    .ok()?;
                Some(RenderDrawVertex::new(
                    [target.x / width * 2.0 - 1.0, 1.0 - target.y / height * 2.0],
                    vertex.color,
                    vertex.uv,
                ))
            })
            .collect();
    }
    let mut output = Vec::new();
    for triangle in mesh.indices.chunks_exact(3) {
        let Some(vertices) = triangle
            .iter()
            .map(|&index| mesh.vertices.get(index as usize))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let polygon = vertices
            .iter()
            .map(|vertex| UiLogicalVertex {
                position: [
                    item.rect.x + vertex.position[0],
                    item.rect.y + vertex.position[1],
                ],
                uv: vertex.uv,
                color: vertex.color,
            })
            .collect();
        let polygon = transform_item_polygon(polygon, item.presentation);
        let polygon = clip_polygon_chain_presented(
            polygon,
            &clips,
            canvas_pixel_scale(presentation, &item.canvas_id),
        );
        output.extend(project_polygon(
            presentation,
            item.canvas_id.as_str(),
            &polygon,
            [1.0; 4],
        ));
    }
    output
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_ui_projected_batch(
    batches: &mut Vec<UiProjectedOrderedBatch>,
    key: UiProjectedBatchKey,
    texture: Option<RenderResourceHandle>,
    font_render_mode: Option<FontBundleRenderMode>,
    font_page_index: Option<u32>,
    glyph_count: usize,
    debug_label: String,
    vertices: Vec<RenderDrawVertex>,
) {
    if let Some(last) = batches.last_mut().filter(|batch| batch.key == key) {
        last.glyph_count += glyph_count;
        last.vertices
            .extend_chunks_from(&UiVertexStream::from(vertices));
        return;
    }
    batches.push(UiProjectedOrderedBatch {
        key,
        texture,
        font_render_mode,
        font_page_index,
        glyph_count,
        debug_label,
        vertices: vertices.into(),
        retained: Vec::new(),
    });
}

#[derive(Debug, Clone, Copy)]
pub(super) struct UiLogicalVertex {
    pub(super) position: [f32; 2],
    pub(super) uv: [f32; 2],
    pub(super) color: [f32; 4],
}

pub(super) fn append_resolved_paint_batches(
    output: &mut UiProjectedOrderedBatches,
    item: &crate::aui::AuiOverlayDrawItem,
    paint: &crate::aui_theme::AuiResolvedPaint,
    presentation: &ResolvedGameViewPresentation,
    texture_bindings: Option<&RuntimeTextureBindingContext>,
) {
    if paint.shadow.visible() {
        if let Some(mut color) = paint
            .shadow
            .color
            .as_deref()
            .and_then(|value| parse_aui_color(Some(value)))
        {
            color[3] *= paint.opacity / 4.0;
            for step in (1..=4).rev() {
                let blur = paint.shadow.blur * step as f32 / 4.0;
                let extent = paint.shadow.spread + blur;
                let rect = inset_rect(
                    crate::aui::AuiComputedRect {
                        x: item.rect.x + paint.shadow.offset_x,
                        y: item.rect.y + paint.shadow.offset_y,
                        width: item.rect.width,
                        height: item.rect.height,
                    },
                    -extent,
                );
                let vertices = projected_rounded_rect_vertices(
                    presentation,
                    item.canvas_id.as_str(),
                    rect,
                    paint.radius + extent,
                    item,
                    color,
                );
                if !vertices.is_empty() {
                    append_ui_projected_batch(
                        &mut output.batches,
                        UiProjectedBatchKey::Solid,
                        None,
                        None,
                        None,
                        0,
                        "shadow".into(),
                        vertices,
                    );
                }
            }
        }
    }

    let border_width = paint
        .border
        .width
        .max(0.0)
        .min(item.rect.width.min(item.rect.height) * 0.5);
    if border_width > 0.0 {
        if let Some(mut color) = paint
            .border
            .color
            .as_deref()
            .and_then(|value| parse_aui_color(Some(value)))
        {
            color[3] *= paint.opacity;
            let vertices = projected_rounded_ring_vertices(
                presentation,
                item.canvas_id.as_str(),
                item.rect,
                paint.radius,
                border_width,
                item,
                color,
            );
            if !vertices.is_empty() {
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Solid,
                    None,
                    None,
                    None,
                    0,
                    "border".into(),
                    vertices,
                );
            }
        }
    }

    let content_rect = inset_rect(item.rect, border_width);
    let content_radius = (paint.radius - border_width).max(0.0);
    match &paint.background {
        crate::aui_theme::AuiResolvedBrush::None => {}
        crate::aui_theme::AuiResolvedBrush::Solid { color } => {
            let Some(mut color) = parse_aui_color(Some(color)) else {
                return;
            };
            color[3] *= paint.opacity;
            let vertices = projected_rounded_rect_vertices(
                presentation,
                item.canvas_id.as_str(),
                content_rect,
                content_radius,
                item,
                color,
            );
            if !vertices.is_empty() {
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Solid,
                    None,
                    None,
                    None,
                    0,
                    "background".into(),
                    vertices,
                );
            }
        }
        crate::aui_theme::AuiResolvedBrush::Image { asset_id, tint } => {
            let Some(binding) = texture_bindings.and_then(|bindings| bindings.get(asset_id)) else {
                output.diagnostics.push(RenderGraphDiagnostic::error(
                    "aui_theme.texture_not_resolved",
                    format!("Theme image brush could not resolve texture '{asset_id}'."),
                ));
                return;
            };
            let mut clips = resolved_item_clips(item);
            if content_radius > 0.0 {
                clips.push(crate::aui::AuiResolvedClip {
                    transform: item.presentation.map(|p| p.vertex_transform),
                    node_id: item.node_id.clone(),
                    rect: content_rect,
                    radius: content_radius,
                });
            }
            let mut color = parse_aui_color(Some(tint)).unwrap_or([1.0; 4]);
            color[3] *= paint.opacity;
            let vertices = projected_quad_with_presentation(
                presentation,
                item.canvas_id.as_str(),
                content_rect,
                &clips,
                color,
                [0.0, 0.0, 1.0, 1.0],
                item.presentation,
            );
            if !vertices.is_empty() {
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Texture(binding.handle),
                    Some(binding.handle),
                    None,
                    None,
                    0,
                    format!("theme-image:{asset_id}"),
                    vertices,
                );
            }
        }
        crate::aui_theme::AuiResolvedBrush::NineSlice {
            asset_id,
            tint,
            slice,
        } => {
            let Some(binding) = texture_bindings.and_then(|bindings| bindings.get(asset_id)) else {
                output.diagnostics.push(RenderGraphDiagnostic::error(
                    "aui_theme.texture_not_resolved",
                    format!("Theme nine-slice brush could not resolve texture '{asset_id}'."),
                ));
                return;
            };
            let mut color = parse_aui_color(Some(tint)).unwrap_or([1.0; 4]);
            color[3] *= paint.opacity;
            let mut clips = resolved_item_clips(item);
            if content_radius > 0.0 {
                clips.push(crate::aui::AuiResolvedClip {
                    transform: item.presentation.map(|p| p.vertex_transform),
                    node_id: item.node_id.clone(),
                    rect: content_rect,
                    radius: content_radius,
                });
            }
            let vertices = projected_nine_slice_vertices_presented(
                presentation,
                item.canvas_id.as_str(),
                content_rect,
                *slice,
                binding.width,
                binding.height,
                color,
                &clips,
                item.presentation,
            );
            if !vertices.is_empty() {
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Texture(binding.handle),
                    Some(binding.handle),
                    None,
                    None,
                    0,
                    format!("nine-slice:{asset_id}"),
                    vertices,
                );
            }
        }
    }
}

pub(super) fn inset_rect(
    rect: crate::aui::AuiComputedRect,
    inset: f32,
) -> crate::aui::AuiComputedRect {
    crate::aui::AuiComputedRect {
        x: rect.x + inset,
        y: rect.y + inset,
        width: (rect.width - inset * 2.0).max(0.0),
        height: (rect.height - inset * 2.0).max(0.0),
    }
}

pub(super) fn projected_rounded_rect_vertices(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    radius: f32,
    item: &crate::aui::AuiOverlayDrawItem,
    color: [f32; 4],
) -> Vec<RenderDrawVertex> {
    let mut polygon =
        rounded_rect_polygon_precise(rect, radius, item_pixel_scale(presentation, item));
    polygon = transform_item_polygon(polygon, item.presentation);
    polygon = clip_polygon_chain_presented(
        polygon,
        &resolved_item_clips(item),
        canvas_pixel_scale(presentation, canvas_id),
    );
    project_polygon(presentation, canvas_id, &polygon, color)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn projected_rounded_ring_vertices(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    radius: f32,
    width: f32,
    item: &crate::aui::AuiOverlayDrawItem,
    color: [f32; 4],
) -> Vec<RenderDrawVertex> {
    let width = width.max(0.0).min(rect.width.min(rect.height) * 0.5);
    if width <= f32::EPSILON {
        return Vec::new();
    }
    let inner_rect = inset_rect(rect, width);
    if inner_rect.width <= f32::EPSILON || inner_rect.height <= f32::EPSILON {
        return projected_rounded_rect_vertices(presentation, canvas_id, rect, radius, item, color);
    }

    let Some(segments) = rounded_rect_segments(rect, radius, item_pixel_scale(presentation, item))
    else {
        return Vec::new();
    };
    // Shared angular samples keep both sides paired even when the inner radius is zero.
    let outer = rounded_rect_outline_segments(rect, radius, segments);
    let inner = rounded_rect_outline_segments(inner_rect, (radius - width).max(0.0), segments);
    let clips = resolved_item_clips(item);
    let mut output = Vec::with_capacity(outer.len() * 6);
    for index in 0..outer.len() {
        let next = (index + 1) % outer.len();
        for triangle in [
            vec![outer[index], outer[next], inner[next]],
            vec![outer[index], inner[next], inner[index]],
        ] {
            let triangle = transform_item_polygon(triangle, item.presentation);
            let polygon = clip_polygon_chain_presented(
                triangle,
                &clips,
                canvas_pixel_scale(presentation, canvas_id),
            );
            output.extend(project_polygon(presentation, canvas_id, &polygon, color));
        }
    }
    output
}

pub(super) fn projected_clipped_quad_vertices(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    item: &crate::aui::AuiOverlayDrawItem,
    color: [f32; 4],
    uv: [f32; 4],
) -> Vec<RenderDrawVertex> {
    projected_quad_with_presentation(
        presentation,
        canvas_id,
        rect,
        &resolved_item_clips(item),
        color,
        uv,
        item.presentation,
    )
}

pub(super) fn resolved_item_clips(
    item: &crate::aui::AuiOverlayDrawItem,
) -> Vec<crate::aui::AuiResolvedClip> {
    if !item.effective_clips.is_empty() {
        return item.effective_clips.clone();
    }
    item.effective_clip_rect
        .map(|rect| {
            vec![crate::aui::AuiResolvedClip {
                transform: None,
                node_id: String::new(),
                rect,
                radius: 0.0,
            }]
        })
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn projected_quad_with_presentation(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    clips: &[crate::aui::AuiResolvedClip],
    color: [f32; 4],
    [u0, v0, u1, v1]: [f32; 4],
    geometry: Option<crate::aui::AuiPresentationGeometry>,
) -> Vec<RenderDrawVertex> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Vec::new();
    }
    let polygon = vec![
        UiLogicalVertex {
            position: [rect.x, rect.y],
            uv: [u0, v0],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [rect.x + rect.width, rect.y],
            uv: [u1, v0],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [rect.x + rect.width, rect.y + rect.height],
            uv: [u1, v1],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [rect.x, rect.y + rect.height],
            uv: [u0, v1],
            color: [1.0; 4],
        },
    ];
    let polygon = transform_item_polygon(polygon, geometry);
    let polygon =
        clip_polygon_chain_presented(polygon, clips, canvas_pixel_scale(presentation, canvas_id));
    project_polygon(presentation, canvas_id, &polygon, color)
}

#[cfg(test)]
pub(super) fn projected_nine_slice_vertices(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    slice: crate::aui_theme::AuiSliceInsets,
    texture_width: u32,
    texture_height: u32,
    color: [f32; 4],
    clips: &[crate::aui::AuiResolvedClip],
) -> Vec<RenderDrawVertex> {
    projected_nine_slice_vertices_presented(
        presentation,
        canvas_id,
        rect,
        slice,
        texture_width,
        texture_height,
        color,
        clips,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn projected_nine_slice_vertices_presented(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
    slice: crate::aui_theme::AuiSliceInsets,
    texture_width: u32,
    texture_height: u32,
    color: [f32; 4],
    clips: &[crate::aui::AuiResolvedClip],
    geometry: Option<crate::aui::AuiPresentationGeometry>,
) -> Vec<RenderDrawVertex> {
    let (left, right) = clamp_pair(slice.left, slice.right, rect.width);
    let (top, bottom) = clamp_pair(slice.top, slice.bottom, rect.height);
    let (source_left, source_right) = clamp_pair(slice.left, slice.right, texture_width as f32);
    let (source_top, source_bottom) = clamp_pair(slice.top, slice.bottom, texture_height as f32);
    let xs = [
        rect.x,
        rect.x + left,
        rect.x + rect.width - right,
        rect.x + rect.width,
    ];
    let ys = [
        rect.y,
        rect.y + top,
        rect.y + rect.height - bottom,
        rect.y + rect.height,
    ];
    let us = [
        0.0,
        source_left / texture_width as f32,
        1.0 - source_right / texture_width as f32,
        1.0,
    ];
    let vs = [
        0.0,
        source_top / texture_height as f32,
        1.0 - source_bottom / texture_height as f32,
        1.0,
    ];
    let mut vertices = Vec::new();
    for y in 0..3 {
        for x in 0..3 {
            let piece = crate::aui::AuiComputedRect {
                x: xs[x],
                y: ys[y],
                width: xs[x + 1] - xs[x],
                height: ys[y + 1] - ys[y],
            };
            vertices.extend(projected_quad_with_presentation(
                presentation,
                canvas_id,
                piece,
                clips,
                color,
                [us[x], vs[y], us[x + 1], vs[y + 1]],
                geometry,
            ));
        }
    }
    vertices
}

pub(super) fn clamp_pair(first: f32, second: f32, available: f32) -> (f32, f32) {
    let first = first.max(0.0);
    let second = second.max(0.0);
    let total = first + second;
    if total <= available || total <= f32::EPSILON {
        (first, second)
    } else {
        let scale = available.max(0.0) / total;
        (first * scale, second * scale)
    }
}

pub(super) fn rounded_rect_polygon(
    rect: crate::aui::AuiComputedRect,
    radius: f32,
) -> Vec<UiLogicalVertex> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Vec::new();
    }
    let radius = radius.max(0.0).min(rect.width.min(rect.height) * 0.5);
    if radius <= f32::EPSILON {
        return vec![
            UiLogicalVertex {
                position: [rect.x, rect.y],
                uv: [0.0, 0.0],
                color: [1.0; 4],
            },
            UiLogicalVertex {
                position: [rect.x + rect.width, rect.y],
                uv: [1.0, 0.0],
                color: [1.0; 4],
            },
            UiLogicalVertex {
                position: [rect.x + rect.width, rect.y + rect.height],
                uv: [1.0, 1.0],
                color: [1.0; 4],
            },
            UiLogicalVertex {
                position: [rect.x, rect.y + rect.height],
                uv: [0.0, 1.0],
                color: [1.0; 4],
            },
        ];
    }
    rounded_rect_outline(rect, radius)
}

pub(super) fn canvas_pixel_scale(presentation: &ResolvedGameViewPresentation, canvas: &str) -> f32 {
    presentation
        .reference_rect_to_target(canvas, GameViewRect::new(0., 0., 1., 1.))
        .map_or(1.0, |r| r.width.max(r.height))
}
pub(super) fn item_pixel_scale(
    presentation: &ResolvedGameViewPresentation,
    item: &crate::aui::AuiOverlayDrawItem,
) -> f32 {
    item.presentation
        .and_then(|p| {
            project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(p.vertex_transform)
                .ok()
        })
        .map_or(1.0, |m| m.maximum_scale() as f32)
        * canvas_pixel_scale(presentation, &item.canvas_id)
}
pub(super) fn rounded_rect_polygon_precise(
    rect: crate::aui::AuiComputedRect,
    radius: f32,
    scale: f32,
) -> Vec<UiLogicalVertex> {
    let radius = radius.max(0.).min(rect.width.min(rect.height) * 0.5);
    if radius <= f32::EPSILON {
        return rounded_rect_polygon(rect, 0.);
    }
    let Some(segments) = rounded_rect_segments(rect, radius, scale) else {
        return Vec::new();
    };
    rounded_rect_outline_segments(rect, radius, segments)
}

pub(super) fn rounded_rect_segments(
    rect: crate::aui::AuiComputedRect,
    radius: f32,
    scale: f32,
) -> Option<usize> {
    let radius = radius.max(0.).min(rect.width.min(rect.height) * 0.5);
    let radius_pixels = f64::from(radius) * f64::from(scale);
    if radius_pixels <= 0.25 {
        return Some(1);
    }
    let angle = 2.0 * (1.0 - 0.25 / radius_pixels).clamp(-1., 1.).acos();
    let segments = (std::f64::consts::FRAC_PI_2 / angle).ceil();
    if !segments.is_finite() || segments > 256. {
        return None;
    }
    Some(segments.max(1.) as usize)
}

pub(super) fn rounded_rect_outline(
    rect: crate::aui::AuiComputedRect,
    radius: f32,
) -> Vec<UiLogicalVertex> {
    rounded_rect_outline_segments(rect, radius, 6)
}

pub(super) fn rounded_rect_outline_segments(
    rect: crate::aui::AuiComputedRect,
    radius: f32,
    segments: usize,
) -> Vec<UiLogicalVertex> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Vec::new();
    }
    let radius = radius.max(0.0).min(rect.width.min(rect.height) * 0.5);
    let corners = [
        (
            [rect.x + radius, rect.y + radius],
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ),
        (
            [rect.x + rect.width - radius, rect.y + radius],
            std::f32::consts::PI * 1.5,
            std::f32::consts::PI * 2.0,
        ),
        (
            [rect.x + rect.width - radius, rect.y + rect.height - radius],
            0.0,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            [rect.x + radius, rect.y + rect.height - radius],
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ),
    ];
    let mut output = Vec::with_capacity(4 * (segments + 1));
    for (center, start, end) in corners {
        for step in 0..=segments {
            let angle = start + (end - start) * step as f32 / segments as f32;
            let x = center[0] + angle.cos() * radius;
            let y = center[1] + angle.sin() * radius;
            output.push(UiLogicalVertex {
                position: [x, y],
                uv: [(x - rect.x) / rect.width, (y - rect.y) / rect.height],
                color: [1.0; 4],
            });
        }
    }
    output
}

#[cfg(test)]
pub(super) fn clip_polygon_chain(
    subject: Vec<UiLogicalVertex>,
    clips: &[crate::aui::AuiResolvedClip],
) -> Vec<UiLogicalVertex> {
    clip_polygon_chain_presented(subject, clips, 1.0)
}

pub(super) fn clip_polygon_chain_presented(
    mut subject: Vec<UiLogicalVertex>,
    clips: &[crate::aui::AuiResolvedClip],
    canvas_scale: f32,
) -> Vec<UiLogicalVertex> {
    // Eight ancestor clips plus the image brush's own rounded content boundary.
    if clips.len() > 9 {
        return Vec::new();
    }
    for clip in clips {
        let scale = clip
            .transform
            .and_then(|m| {
                project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(m).ok()
            })
            .map_or(1.0, |m| m.maximum_scale() as f32)
            * canvas_scale;
        let mut boundary = rounded_rect_polygon_precise(clip.rect, clip.radius, scale);
        if let Some(matrix) = clip.transform {
            let Ok(matrix) =
                project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(matrix)
            else {
                return Vec::new();
            };
            if matrix.try_inverse().is_err() {
                return Vec::new();
            }
            for vertex in &mut boundary {
                let Ok(point) = matrix.transform_point(vertex.position) else {
                    return Vec::new();
                };
                vertex.position = point;
            }
        }
        subject = clip_convex_polygon(subject, &boundary);
        if subject.len() < 3 {
            break;
        }
    }
    subject
}

pub(super) fn transform_item_polygon(
    mut polygon: Vec<UiLogicalVertex>,
    geometry: Option<crate::aui::AuiPresentationGeometry>,
) -> Vec<UiLogicalVertex> {
    let Some(geometry) = geometry else {
        return polygon;
    };
    let Ok(matrix) = project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(
        geometry.vertex_transform,
    ) else {
        return Vec::new();
    };
    for vertex in &mut polygon {
        let Ok(point) = matrix.transform_point(vertex.position) else {
            return Vec::new();
        };
        vertex.position = point;
        for (channel, tint) in vertex.color.iter_mut().zip(geometry.modulation) {
            *channel *= tint;
        }
    }
    polygon
}

pub(super) fn clip_convex_polygon(
    mut subject: Vec<UiLogicalVertex>,
    boundary: &[UiLogicalVertex],
) -> Vec<UiLogicalVertex> {
    if subject.len() < 3 || boundary.len() < 3 {
        return Vec::new();
    }
    let orientation = polygon_area(boundary).signum();
    for edge in 0..boundary.len() {
        let a = boundary[edge].position;
        let b = boundary[(edge + 1) % boundary.len()].position;
        let input = std::mem::take(&mut subject);
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let mut previous_inside = edge_inside(previous.position, a, b, orientation);
        for current in input {
            let current_inside = edge_inside(current.position, a, b, orientation);
            if current_inside != previous_inside {
                subject.push(segment_edge_intersection(previous, current, a, b));
            }
            if current_inside {
                subject.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
        if subject.is_empty() {
            break;
        }
    }
    subject
}

pub(super) fn polygon_area(vertices: &[UiLogicalVertex]) -> f32 {
    vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| a.position[0] * b.position[1] - b.position[0] * a.position[1])
        .sum::<f32>()
        * 0.5
}

pub(super) fn edge_inside(point: [f32; 2], a: [f32; 2], b: [f32; 2], orientation: f32) -> bool {
    let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
    cross * orientation >= -0.0001
}

pub(super) fn segment_edge_intersection(
    from: UiLogicalVertex,
    to: UiLogicalVertex,
    a: [f32; 2],
    b: [f32; 2],
) -> UiLogicalVertex {
    let direction = [
        to.position[0] - from.position[0],
        to.position[1] - from.position[1],
    ];
    let edge = [b[0] - a[0], b[1] - a[1]];
    let denominator = direction[0] * edge[1] - direction[1] * edge[0];
    let t = if denominator.abs() <= f32::EPSILON {
        0.0
    } else {
        ((a[0] - from.position[0]) * edge[1] - (a[1] - from.position[1]) * edge[0]) / denominator
    }
    .clamp(0.0, 1.0);
    UiLogicalVertex {
        position: [
            from.position[0] + direction[0] * t,
            from.position[1] + direction[1] * t,
        ],
        uv: [
            from.uv[0] + (to.uv[0] - from.uv[0]) * t,
            from.uv[1] + (to.uv[1] - from.uv[1]) * t,
        ],
        color: std::array::from_fn(|i| from.color[i] + (to.color[i] - from.color[i]) * t),
    }
}

pub(super) fn project_polygon(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    polygon: &[UiLogicalVertex],
    color: [f32; 4],
) -> Vec<RenderDrawVertex> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let Some(extent) = presentation.canvas_reference_extent(canvas_id) else {
        return Vec::new();
    };
    let canvas_rect = crate::aui::AuiComputedRect {
        x: 0.,
        y: 0.,
        width: extent.width as f32,
        height: extent.height as f32,
    };
    // Mapping rejects out-of-canvas points. Clip the polygon before triangulation so
    // a crossing vertex cannot discard visible triangles. In-bounds polygons stay borrowed.
    let polygon = if polygon
        .iter()
        .all(|vertex| canvas_rect.contains(vertex.position[0], vertex.position[1]))
    {
        std::borrow::Cow::Borrowed(polygon)
    } else {
        let mut clipped =
            clip_convex_polygon(polygon.to_vec(), &rounded_rect_polygon(canvas_rect, 0.));
        // Remove only numerical edge overshoot after interpolating UV and RGBA.
        for vertex in &mut clipped {
            vertex.position[0] = vertex.position[0].clamp(0., canvas_rect.width);
            vertex.position[1] = vertex.position[1].clamp(0., canvas_rect.height);
        }
        std::borrow::Cow::Owned(clipped)
    };
    if polygon.len() < 3 {
        return Vec::new();
    }
    let width = presentation.target_extent.width.max(1) as f32;
    let height = presentation.target_extent.height.max(1) as f32;
    let project = |vertex: UiLogicalVertex| {
        let target = presentation
            .reference_rect_to_target(
                canvas_id,
                GameViewRect::new(vertex.position[0], vertex.position[1], 0.0, 0.0),
            )
            .ok()?;
        Some(RenderDrawVertex::new(
            [target.x / width * 2.0 - 1.0, 1.0 - target.y / height * 2.0],
            std::array::from_fn(|i| (color[i] * vertex.color[i]).clamp(0., 1.)),
            vertex.uv,
        ))
    };
    let Some(origin) = project(polygon[0]) else {
        return Vec::new();
    };
    let mut output = Vec::with_capacity((polygon.len() - 2) * 3);
    for index in 1..polygon.len() - 1 {
        let (Some(a), Some(b)) = (project(polygon[index]), project(polygon[index + 1])) else {
            continue;
        };
        output.extend([origin, a, b]);
    }
    output
}

pub(super) fn ui_projection_image_vertices(
    item: &crate::aui::AuiOverlayDrawItem,
    presentation: &ResolvedGameViewPresentation,
) -> Option<Vec<RenderDrawVertex>> {
    let color = item
        .color
        .as_deref()
        .and_then(|value| parse_aui_color(Some(value)))
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    Some(projected_clipped_quad_vertices(
        presentation,
        item.canvas_id.as_str(),
        item.rect,
        item,
        color,
        [0.0, 0.0, 1.0, 1.0],
    ))
}

pub(super) fn resolve_ui_presentation(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    output_extent: [f32; 2],
    target_id: &str,
    scale_policy: GameViewScalePolicy,
) -> Result<ResolvedGameViewPresentation, &'static str> {
    let target_extent = GameViewExtent::new(
        output_extent[0].round().max(1.0) as u32,
        output_extent[1].round().max(1.0) as u32,
    );
    let stage_canvas_ids = stage
        .draw_items
        .iter()
        .map(|item| item.canvas_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut canvas_references = composition
        .canvas_references
        .iter()
        .filter(|fact| stage_canvas_ids.contains(fact.canvas_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let known = canvas_references
        .iter()
        .map(|fact| fact.canvas_id.clone())
        .collect::<BTreeSet<_>>();
    for canvas_id in stage_canvas_ids {
        if !known.contains(canvas_id) {
            canvas_references.push(CanvasReferenceFact {
                canvas_id: canvas_id.to_string(),
                reference_extent: target_extent,
            });
        }
    }
    GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: format!("runtime-frame-{}", composition.frame_index),
        target_id: target_id.to_string(),
        target_extent,
        display_rect: GameViewRect::from_extent(target_extent),
        scale_policy,
        surface_generation: 1,
        presentation_revision: composition.frame_index,
        canvas_references,
    })
    .map_err(|error| error.code)
}

pub fn ui_projection_font_batches(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
) -> Vec<UiProjectedFontBatch> {
    let Some(plan) = composition.glyph_plan.as_ref() else {
        return Vec::new();
    };
    let item_ids = stage
        .draw_items
        .iter()
        .map(|item| item.item_id.as_str())
        .collect::<HashSet<_>>();
    let item_facts = stage
        .draw_items
        .iter()
        .map(|item| (item.item_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<(String, u64, FontBundleRenderMode, u32), Vec<_>>::new();
    for glyph in plan
        .quads
        .iter()
        .filter(|glyph| item_ids.contains(glyph.item_id.as_str()))
    {
        grouped
            .entry((
                glyph
                    .font_bundle
                    .as_ref()
                    .map(|b| b.0.clone())
                    .unwrap_or_else(|| plan.font_atlas_id.clone()),
                glyph
                    .font_bundle
                    .as_ref()
                    .map(|b| b.1)
                    .unwrap_or(plan.atlas_generation),
                glyph.render_mode,
                glyph.page_index,
            ))
            .or_default()
            .push(glyph);
    }
    grouped
        .into_iter()
        .map(
            |((bundle_id, generation, render_mode, page_index), glyphs)| {
                let mut vertices = Vec::new();
                for glyph in &glyphs {
                    let Some(item) = item_facts.get(glyph.item_id.as_str()) else {
                        continue;
                    };
                    let color = parse_aui_color(glyph.color.as_deref().or(item.color.as_deref()))
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let [u0, v0, u1, v1] = glyph.uv_rect;
                    if resolved_item_clips(item).is_empty() && item.presentation.is_none() {
                        if let Some(positions) = projected_ui_rect_positions(
                            presentation,
                            item.canvas_id.as_str(),
                            glyph.rect,
                        ) {
                            vertices.extend(quad_vertices(
                                positions,
                                color,
                                [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                            ));
                        }
                    } else {
                        vertices.extend(projected_clipped_quad_vertices(
                            presentation,
                            item.canvas_id.as_str(),
                            glyph.rect,
                            item,
                            color,
                            [u0, v0, u1, v1],
                        ));
                    }
                }
                let formal_v2 = uses_font_bundle_pages(&plan.font_source_kind);
                UiProjectedFontBatch {
                    render_mode,
                    page_index,
                    glyph_count: glyphs.len(),
                    texture: if formal_v2 {
                        font_bundle_page_generation_render_handle(
                            &bundle_id,
                            render_mode,
                            page_index,
                            generation,
                        )
                    } else {
                        font_atlas_render_handle(&plan.font_atlas_id)
                    },
                    font_render_mode: formal_v2.then_some(render_mode),
                    vertices,
                }
            },
        )
        .collect()
}

pub fn ui_projection_geometry_vertices(
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
) -> Vec<RenderDrawVertex> {
    let mut vertices = Vec::new();
    for item in stage.draw_items.iter() {
        if item.item_kind == AuiOverlayItemKind::Mesh {
            if let Some(mesh) = item.mesh.as_ref() {
                vertices.extend(projected_custom_mesh_vertices(item, mesh, presentation));
            }
            continue;
        }
        if !matches!(
            item.item_kind,
            AuiOverlayItemKind::Rect
                | AuiOverlayItemKind::ScrollbarTrack
                | AuiOverlayItemKind::ScrollbarThumb
        ) {
            continue;
        }
        if let Some(paint) = item.paint.as_ref() {
            let border_width = paint
                .border
                .width
                .max(0.0)
                .min(item.rect.width.min(item.rect.height) * 0.5);
            if let Some(mut color) = paint
                .border
                .color
                .as_deref()
                .and_then(|value| parse_aui_color(Some(value)))
            {
                color[3] *= paint.opacity;
                vertices.extend(projected_rounded_ring_vertices(
                    presentation,
                    item.canvas_id.as_str(),
                    item.rect,
                    paint.radius,
                    border_width,
                    item,
                    color,
                ));
            }
            if let crate::aui_theme::AuiResolvedBrush::Solid { color } = &paint.background {
                if let Some(mut color) = parse_aui_color(Some(color)) {
                    color[3] *= paint.opacity;
                    vertices.extend(projected_rounded_rect_vertices(
                        presentation,
                        item.canvas_id.as_str(),
                        inset_rect(item.rect, border_width),
                        (paint.radius - border_width).max(0.0),
                        item,
                        color,
                    ));
                }
            }
            continue;
        }
        let Some(color) = parse_aui_color(item.color.as_deref()) else {
            continue;
        };
        vertices.extend(projected_clipped_quad_vertices(
            presentation,
            item.canvas_id.as_str(),
            item.rect,
            item,
            color,
            [0.0, 0.0, 1.0, 1.0],
        ));
    }
    vertices
}

pub(super) fn projected_ui_rect_positions(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
) -> Option<[[f32; 2]; 4]> {
    let target = presentation
        .reference_rect_to_target(
            canvas_id,
            GameViewRect::new(rect.x, rect.y, rect.width, rect.height),
        )
        .ok()?;
    let width = presentation.target_extent.width.max(1) as f32;
    let height = presentation.target_extent.height.max(1) as f32;
    let x0 = target.x / width * 2.0 - 1.0;
    let x1 = (target.x + target.width) / width * 2.0 - 1.0;
    let y0 = 1.0 - target.y / height * 2.0;
    let y1 = 1.0 - (target.y + target.height) / height * 2.0;
    Some([[x0, y1], [x1, y1], [x1, y0], [x0, y0]])
}

pub(super) fn parse_aui_color(value: Option<&str>) -> Option<[f32; 4]> {
    let value = value?.strip_prefix('#')?;
    if value.len() != 6 && value.len() != 8 {
        return None;
    }
    let channel = |offset| u8::from_str_radix(&value[offset..offset + 2], 16).ok();
    let rgb = |offset| {
        channel(offset).map(|byte| {
            project_runtime_sdk::aui_presentation::srgb_channel_to_linear(f32::from(byte) / 255.0)
        })
    };
    Some([
        rgb(0)?,
        rgb(2)?,
        rgb(4)?,
        value
            .get(6..8)
            .map(|_| channel(6).map(|alpha| f32::from(alpha) / 255.0))
            .unwrap_or(Some(1.0))?,
    ])
}

pub(super) fn quad_vertices(
    positions: [[f32; 2]; 4],
    color: [f32; 4],
    uvs: [[f32; 2]; 4],
) -> Vec<RenderDrawVertex> {
    [0usize, 1, 2, 0, 2, 3]
        .into_iter()
        .map(|index| RenderDrawVertex::new(positions[index], color, uvs[index]))
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RuntimeUiCompositionStageMetrics {
    pub(super) stage_count: usize,
    pub(super) before_world_item_count: usize,
    pub(super) screen_overlay_item_count: usize,
    pub(super) modal_item_count: usize,
    pub(super) before_world_pass_present: bool,
    pub(super) screen_overlay_pass_present: bool,
    pub(super) modal_pass_present: bool,
}

impl RuntimeUiCompositionStageMetrics {
    pub(super) fn from_graph(graph: &RenderGraph) -> Self {
        let mut metrics = Self::default();
        for command in graph.passes.iter().flat_map(|pass| pass.commands.iter()) {
            let (RenderPassCommand::DrawUiComposition {
                stage, item_count, ..
            }
            | RenderPassCommand::DrawUiRetained {
                stage, item_count, ..
            }) = command
            else {
                continue;
            };
            metrics.stage_count += 1;
            match stage.as_str() {
                "BeforeWorld" => {
                    metrics.before_world_item_count += *item_count;
                    metrics.before_world_pass_present = true;
                }
                "ScreenOverlay" => {
                    metrics.screen_overlay_item_count += *item_count;
                    metrics.screen_overlay_pass_present = true;
                }
                "Modal" => {
                    metrics.modal_item_count += *item_count;
                    metrics.modal_pass_present = true;
                }
                _ => {}
            }
        }
        metrics
    }
}
