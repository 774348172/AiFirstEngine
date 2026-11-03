//! Bounded previous-frame products shared by base and feedback presentation.
use super::*;
use crate::aui_control_feedback::AuiControlVisualOverride;
use crate::aui_paragraph::ParagraphLayout;
use crate::game_view_presentation::GameViewRect;

pub(super) const MAX_DRAW_NODES: usize = 16_384;
pub(super) const MAX_GLYPH_QUADS: usize = 65_536;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiDrawingStats {
    pub index_rebuilds: u64,
    pub index_entries_built: u64,
    pub nodes_compared: u64,
    /// Nodes actually entered by change discovery/update, excluding visible submission.
    pub nodes_visited: u64,
    pub command_refs_submitted: u64,
    pub items_visited: u64,
    pub item_refs_copied: u64,
    pub text_items_visited: u64,
    pub quad_refs_submitted: u64,
    pub glyph_hash_fragments_visited: u64,
    pub glyph_hash_structure_rebuilds: u64,
    pub nodes_built: u64,
    pub nodes_reused: u64,
    pub command_payloads_built: u64,
    pub command_payloads_reused: u64,
    pub item_payloads_built: u64,
    pub item_payloads_reused: u64,
    pub text_items_compared: u64,
    pub text_items_built: u64,
    pub text_items_reused: u64,
    pub quad_payloads_built: u64,
    pub quad_payloads_reused: u64,
    pub glyph_hash_leaves_built: u64,
    pub glyph_hash_leaves_reused: u64,
}

impl AuiDrawingStats {
    pub fn since(self, previous: Self) -> Self {
        macro_rules! delta {
            ($($field:ident),* $(,)?) => { Self { $($field: self.$field.saturating_sub(previous.$field)),* } };
        }
        delta!(
            index_rebuilds,
            index_entries_built,
            nodes_compared,
            nodes_visited,
            command_refs_submitted,
            items_visited,
            item_refs_copied,
            text_items_visited,
            quad_refs_submitted,
            glyph_hash_fragments_visited,
            glyph_hash_structure_rebuilds,
            nodes_built,
            nodes_reused,
            command_payloads_built,
            command_payloads_reused,
            item_payloads_built,
            item_payloads_reused,
            text_items_compared,
            text_items_built,
            text_items_reused,
            quad_payloads_built,
            quad_payloads_reused,
            glyph_hash_leaves_built,
            glyph_hash_leaves_reused
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AuiDrawingCache {
    pub(super) graphics: crate::aui_graphics::GraphicsCache,
    pub(super) graphics_errors: BTreeMap<String, crate::aui_graphics::GraphicsError>,
    pub(super) graphics_pixel_scales: BTreeMap<String, f32>,
    graphics_mapping: Option<ResolvedGameViewPresentation>,
    graphics_assets: Option<crate::aui_graphics_asset::GraphicsAssetRegistry>,
    pub(super) indices: DrawingIndices,
    pub(super) nodes: Vec<Option<DrawFragment>>,
    pub(super) visible_slots: BTreeSet<usize>,
    pub(super) command_ranges: BTreeMap<usize, std::ops::Range<usize>>,
    /// Private one-shot product of extraction; used only by the owner's immediate projection.
    pub(super) pending_command_slots: Option<Vec<usize>>,
    pub(super) draw_changes_once: Option<Vec<usize>>,
    pub(super) draw_node_overrides: HashMap<usize, AuiNode>,
    pub(super) feedback_style_targets: BTreeSet<String>,
    pub(super) feedback_overrides: crate::aui_control_feedback::AuiVisualOverrideSet,
    pub(super) feedback_themes: Option<DrawingThemeIdentity>,
    pub(super) item_stage_slots: Vec<(AuiCompositionStage, usize)>,
    pub(super) stage_frames: Vec<AuiCompositionStageFrame>,
    pub(super) stage_command_slots: BTreeMap<AuiCompositionStage, Vec<(usize, usize)>>,
    layout_generation: u64,
    layout_revision: u64,
    layout_node_count: usize,
    pub(super) items: Vec<OverlayFragment>,
    pub(super) glyphs: Vec<GlyphFragment>,
    pub(super) glyph_slots: HashMap<String, usize>,
    pub(super) glyph_presentation: Option<ResolvedGameViewPresentation>,
    pub(super) fonts: Option<RuntimeFontBundleRegistry>,
    raster_index: Option<Arc<C2RasterGlyphIndex>>,
    pub(super) projection_stages: BTreeMap<AuiCompositionStage, Arc<Vec<Arc<AuiOverlayDrawItem>>>>,
    pub(super) pending_projection_stages: BTreeMap<AuiCompositionStage, AuiStageProjectionDelta>,
    pub(super) previous_glyph_plan: Option<Arc<AuiTextGlyphPlan>>,
    pub(super) pending_glyph_groups: Option<BTreeMap<String, Arc<Vec<Arc<AuiTextGlyphQuad>>>>>,
    pub(super) pending_glyph_overlay: Option<Arc<Vec<Arc<AuiOverlayDrawItem>>>>,
    pub(super) glyph_hash: GlyphHashCache,
    pub(crate) stats: AuiDrawingStats,
}

#[derive(Debug, Clone)]
pub(super) struct DrawingThemeIdentity(pub crate::aui_theme::AuiThemeRegistry);

impl PartialEq for DrawingThemeIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.shares_storage(&other.0)
    }
}

impl AuiDrawingCache {
    pub(super) fn prepare_graphics(
        &mut self,
        document: &AuiDocument,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) {
        if !self
            .graphics_assets
            .as_ref()
            .is_some_and(|assets| assets.shares_storage(&document.graphics_assets))
        {
            self.nodes.clear();
            self.draw_changes_once = None;
            self.graphics_assets = Some(document.graphics_assets.clone());
        }
        let mapping_same = match (&self.graphics_mapping, presentation) {
            (None, None) => true,
            (Some(old), Some(new)) => old.same_ui_mapping(new),
            _ => false,
        };
        if mapping_same {
            return;
        }
        let scales: BTreeMap<_, _> = document
            .canvases
            .iter()
            .map(|canvas| {
                let scale = presentation
                    .and_then(|p| {
                        p.reference_rect_to_target(
                            &canvas.canvas_id,
                            GameViewRect::new(0.0, 0.0, 1.0, 1.0),
                        )
                        .ok()
                    })
                    .map_or(1.0, |rect| rect.width.max(rect.height));
                (canvas.canvas_id.clone(), scale)
            })
            .collect();
        if scales != self.graphics_pixel_scales {
            // Mapping changes are rare. Invalidate drawing products, retaining the bounded
            // geometry cache so returning to an earlier DPI can reuse its precision bucket.
            self.nodes.clear();
            self.draw_changes_once = None;
        }
        self.graphics_pixel_scales = scales;
        self.graphics_mapping = presentation.cloned();
    }

    pub(super) fn raster_index(
        &mut self,
        fonts: &RuntimeFontBundleRegistry,
    ) -> Option<Arc<C2RasterGlyphIndex>> {
        if let Some(index) = self
            .raster_index
            .as_ref()
            .filter(|index| Arc::ptr_eq(&index.bundles, &fonts.bundles_by_id))
        {
            return Some(index.clone());
        }
        self.raster_index = C2RasterGlyphIndex::new(fonts).map(Arc::new);
        self.raster_index.clone()
    }

    pub(super) fn changed_layout_slots(&self, layout: &AuiLayoutResult) -> Option<Vec<usize>> {
        if self.nodes.len() != layout.computed_nodes.len() {
            return None;
        }
        Self::changed_slots_since(
            layout,
            (
                self.layout_generation,
                self.layout_revision,
                self.layout_node_count,
            ),
        )
    }

    pub(super) fn changed_slots_since(
        layout: &AuiLayoutResult,
        previous: (u64, u64, usize),
    ) -> Option<Vec<usize>> {
        let update = &layout.interaction_index_update;
        if update.generation == 0
            || previous.0 != update.generation
            || previous.2 != update.node_count
            || update.node_count != layout.computed_nodes.len()
            || previous.1 > update.revision
        {
            return None;
        }
        if previous.1 == update.revision {
            // Public layout/document values can be edited outside the retained owner.
            // Only a newer journal revision proves the exact changed slots.
            return None;
        }

        let first_needed = previous.1.saturating_add(1);
        let first_available = update.history.front()?.revision;
        if first_needed < first_available {
            return None;
        }
        let mut expected = first_needed;
        let mut slots = Vec::new();
        for delta in update
            .history
            .iter()
            .filter(|delta| delta.revision >= first_needed)
        {
            if delta.revision != expected
                || delta.slots.iter().any(|&slot| slot >= update.node_count)
            {
                return None;
            }
            slots.extend(delta.slots.iter().copied());
            expected = expected.saturating_add(1);
        }
        if expected != update.revision.saturating_add(1) {
            return None;
        }
        slots.sort_unstable();
        slots.dedup();
        Some(slots)
    }

    pub(super) fn record_layout(&mut self, layout: &AuiLayoutResult) {
        let update = &layout.interaction_index_update;
        self.layout_generation = update.generation;
        self.layout_revision = update.revision;
        self.layout_node_count = update.node_count;
    }

    pub(super) fn has_recorded_layout(&self, layout: &AuiLayoutResult) -> bool {
        let update = &layout.interaction_index_update;
        update.generation != 0
            && self.layout_generation == update.generation
            && self.layout_revision == update.revision
            && self.layout_node_count == update.node_count
            && update.node_count == layout.computed_nodes.len()
    }

    pub(super) fn clear_layout(&mut self) {
        self.layout_generation = 0;
        self.layout_revision = 0;
        self.layout_node_count = 0;
    }
}

/// Indices retain slot identity, never stale geometry or pointers into caller data.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct DrawingIndices {
    pub nodes: SlotIndex,
    pub computed: SlotIndex,
    pub canvases: SlotIndex,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct SlotIndex {
    ids: Vec<Arc<str>>,
    slots: HashMap<Arc<str>, usize>,
}

impl SlotIndex {
    fn refresh<'a>(&mut self, ids: impl Iterator<Item = &'a str> + Clone) -> bool {
        if ids.clone().eq(self.ids.iter().map(AsRef::as_ref)) {
            return false;
        }
        self.ids = ids.map(Arc::from).collect();
        self.slots = self
            .ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();
        true
    }

    pub fn get(&self, id: &str) -> Option<usize> {
        self.slots.get(id).copied()
    }

    fn trim(&mut self) {
        if self.ids.len() > MAX_DRAW_NODES {
            *self = Self::default();
        }
    }
}

impl DrawingIndices {
    pub fn refresh(&mut self, document: &AuiDocument, layout: &AuiLayoutResult) -> (u64, u64) {
        let mut rebuilds = 0;
        let mut entries = 0;
        if self
            .nodes
            .refresh(document.nodes.iter().map(|n| n.node_id.as_str()))
        {
            rebuilds += 1;
            entries += document.nodes.len() as u64;
        }
        if self
            .computed
            .refresh(layout.computed_nodes.iter().map(|n| n.node_id.as_str()))
        {
            rebuilds += 1;
            entries += layout.computed_nodes.len() as u64;
        }
        if self
            .canvases
            .refresh(document.canvases.iter().map(|c| c.canvas_id.as_str()))
        {
            rebuilds += 1;
            entries += document.canvases.len() as u64;
        }
        (rebuilds, entries)
    }

    pub fn trim(&mut self) {
        self.nodes.trim();
        self.computed.trim();
        self.canvases.trim();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrawFragment {
    pub node: AuiNode,
    pub computed: AuiComputedNode,
    pub visual: Option<AuiControlVisualOverride>,
    pub owner_rect: Option<AuiComputedRect>,
    pub commands: Vec<Arc<AuiDrawCommand>>,
    pub culled_draw_item_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct OverlayFragment {
    pub command: Arc<AuiDrawCommand>,
    pub item: Arc<AuiOverlayDrawItem>,
}

type RasterFaceIndex = HashMap<String, HashMap<(u32, FontBundleRenderMode), Vec<usize>>>;

/// One bounded index per current immutable registry, shared by sparse and full
/// presentation. Entries retain metadata order so nearest-size ties are unchanged.
#[derive(Debug, PartialEq)]
pub(super) struct C2RasterGlyphIndex {
    bundles: Arc<BTreeMap<String, crate::font_bundle::RuntimeLoadedFontBundle>>,
    candidates: HashMap<usize, RasterFaceIndex>,
}

impl C2RasterGlyphIndex {
    pub(super) fn new(fonts: &RuntimeFontBundleRegistry) -> Option<Self> {
        let count = fonts
            .bundles_by_id
            .values()
            .try_fold(0usize, |sum, bundle| {
                sum.checked_add(bundle.metadata.glyphs.len())
            })?;
        if count > MAX_GLYPH_QUADS {
            return None;
        }
        let mut candidates = HashMap::<usize, RasterFaceIndex>::new();
        for bundle in fonts.bundles_by_id.values() {
            // Address is only a safe map key; the owned Arc keeps its source alive.
            let faces = candidates.entry(bundle as *const _ as usize).or_default();
            for (slot, glyph) in bundle.metadata.glyphs.iter().enumerate() {
                faces
                    .entry(glyph.font_face_id.clone())
                    .or_default()
                    .entry((u32::from(glyph.glyph_id), glyph.render_mode))
                    .or_default()
                    .push(slot);
            }
        }
        Some(Self {
            bundles: fonts.bundles_by_id.clone(),
            candidates,
        })
    }

    pub(super) fn candidates(
        &self,
        bundle: &crate::font_bundle::RuntimeLoadedFontBundle,
        face_id: &str,
        glyph_id: u32,
        mode: FontBundleRenderMode,
    ) -> Option<&[usize]> {
        let faces = self.candidates.get(&(bundle as *const _ as usize))?;
        Some(
            faces
                .get(face_id)
                .and_then(|glyphs| glyphs.get(&(glyph_id, mode)))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GlyphFragment {
    pub item: Arc<AuiOverlayDrawItem>,
    pub paragraph: Option<Arc<ParagraphLayout>>,
    pub target_scale: f32,
    pub quads: Vec<Arc<AuiTextGlyphQuad>>,
    pub requested: usize,
    pub unsupported: usize,
    pub clipped: usize,
    pub fallback: bool,
    pub hash: u64,
}

impl GlyphFragment {
    pub fn matches(
        &self,
        item: &Arc<AuiOverlayDrawItem>,
        paragraph: Option<&Arc<ParagraphLayout>>,
        target_scale: f32,
    ) -> bool {
        (Arc::ptr_eq(&self.item, item) || self.item == *item || {
            // Draw ordinals change when a sibling is shown/hidden. They identify
            // publication order, not raster input; every other field still matters.
            let mut raster_input = item.as_ref().clone();
            raster_input.item_id.clone_from(&self.item.item_id);
            raster_input.sort_key = self.item.sort_key;
            self.item.as_ref() == &raster_input
        }) && self.target_scale == target_scale
            && match (&self.paragraph, paragraph) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct GlyphHashCache {
    ids: Vec<String>,
    slots: BTreeMap<String, usize>,
    tree_base: usize,
    tree: Vec<u64>,
}

impl GlyphHashCache {
    pub fn update_slots(
        &mut self,
        fragments: &[GlyphFragment],
        slots: &[usize],
    ) -> (u64, usize, bool) {
        let structure_changed = slots.iter().any(|&slot| {
            let fragment = &fragments[slot];
            fragment.quads.is_empty() == self.slots.contains_key(&fragment.item.item_id)
        });
        if structure_changed {
            self.rebuild(fragments);
        } else {
            for &slot in slots {
                let fragment = &fragments[slot];
                if let Some(&leaf) = self.slots.get(&fragment.item.item_id) {
                    self.set_leaf(leaf, fragment.hash);
                }
            }
        }
        (
            self.root(),
            if structure_changed {
                fragments.len()
            } else {
                slots.len()
            },
            structure_changed,
        )
    }

    pub fn leaf_count(&self) -> usize {
        self.ids.len()
    }
    pub fn update(
        &mut self,
        fragments: &[GlyphFragment],
        changed_groups: &BTreeMap<String, Arc<Vec<Arc<AuiTextGlyphQuad>>>>,
    ) -> (u64, bool) {
        let structure_changed = !self.ids.iter().map(String::as_str).eq(fragments
            .iter()
            .filter(|fragment| !fragment.quads.is_empty())
            .map(|fragment| fragment.item.item_id.as_str()))
            || changed_groups
                .iter()
                .any(|(item_id, quads)| quads.is_empty() == self.slots.contains_key(item_id));
        if structure_changed {
            self.rebuild(fragments);
            return (self.root(), true);
        }
        // Full fallback is allowed to index visible fragments once; do not search
        // all siblings separately for each changed group (e.g. a font replacement).
        let hashes: HashMap<_, _> = fragments
            .iter()
            .map(|fragment| (fragment.item.item_id.as_str(), fragment.hash))
            .collect();
        for (item_id, quads) in changed_groups {
            if quads.is_empty() {
                continue;
            }
            let Some(slot) = self.slots.get(item_id).copied() else {
                self.rebuild(fragments);
                return (self.root(), true);
            };
            let Some(&hash) = hashes.get(item_id.as_str()) else {
                self.rebuild(fragments);
                return (self.root(), true);
            };
            self.set_leaf(slot, hash);
        }
        (self.root(), false)
    }

    fn rebuild(&mut self, fragments: &[GlyphFragment]) {
        self.ids = fragments
            .iter()
            .filter(|fragment| !fragment.quads.is_empty())
            .map(|fragment| fragment.item.item_id.clone())
            .collect();
        self.slots = self
            .ids
            .iter()
            .enumerate()
            .map(|(slot, id)| (id.clone(), slot))
            .collect();
        self.tree_base = self.ids.len().max(1).next_power_of_two();
        self.tree = vec![0; self.tree_base * 2];
        for (slot, fragment) in fragments
            .iter()
            .filter(|fragment| !fragment.quads.is_empty())
            .enumerate()
        {
            self.tree[self.tree_base + slot] = fragment.hash;
        }
        for node in (1..self.tree_base).rev() {
            self.tree[node] =
                super::stable_glyph_hash_pair(self.tree[node * 2], self.tree[node * 2 + 1]);
        }
    }

    fn set_leaf(&mut self, slot: usize, hash: u64) {
        let mut node = self.tree_base + slot;
        self.tree[node] = hash;
        while node > 1 {
            node /= 2;
            self.tree[node] =
                super::stable_glyph_hash_pair(self.tree[node * 2], self.tree[node * 2 + 1]);
        }
    }

    fn root(&self) -> u64 {
        self.tree.get(1).copied().unwrap_or_default()
    }
}
