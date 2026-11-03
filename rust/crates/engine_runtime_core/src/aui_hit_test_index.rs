//! Retained broad-phase index for AUI pointer hit testing.
//! Final geometry remains authoritative; this module only narrows candidates.

use crate::aui::{AuiComputedRect, AuiDocument, AuiLayoutResult, AuiNode, AuiPointer};
use crate::game_view_presentation::{GameViewPoint, ResolvedGameViewPresentation};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const CELL_SIZE: f32 = 128.0;
const MAX_CELLS_PER_NODE: usize = 64;
const MAX_DELTA_HISTORY: usize = 64;
static NEXT_INDEX_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Default)]
pub(crate) struct AuiInteractionIndexUpdate {
    pub generation: u64,
    pub revision: u64,
    pub node_count: usize,
    pub history: Arc<VecDeque<AuiInteractionIndexDelta>>,
}

// This is private cache coordination and is deliberately outside published layout equality.
impl PartialEq for AuiInteractionIndexUpdate {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AuiInteractionIndexDelta {
    pub revision: u64,
    pub slots: Arc<[usize]>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct AuiInteractionIndexJournal {
    current: AuiInteractionIndexUpdate,
}

impl AuiInteractionIndexJournal {
    pub fn reset(&mut self, node_count: usize) -> AuiInteractionIndexUpdate {
        self.current = AuiInteractionIndexUpdate {
            generation: NEXT_INDEX_GENERATION.fetch_add(1, Ordering::Relaxed).max(1),
            revision: 0,
            node_count,
            history: Arc::default(),
        };
        self.current.clone()
    }

    pub fn record(
        &mut self,
        node_count: usize,
        changed_slots: &[usize],
    ) -> AuiInteractionIndexUpdate {
        if self.current.generation == 0 || self.current.node_count != node_count {
            return self.reset(node_count);
        }
        if changed_slots.is_empty() {
            return self.current.clone();
        }
        self.current.revision = self.current.revision.saturating_add(1);
        let history = Arc::make_mut(&mut self.current.history);
        history.push_back(AuiInteractionIndexDelta {
            revision: self.current.revision,
            slots: changed_slots.to_vec().into(),
        });
        while history.len() > MAX_DELTA_HISTORY {
            history.pop_front();
        }
        self.current.clone()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuiHitTestIndexStats {
    pub full_rebuild_count: usize,
    pub node_table_rebuild_count: usize,
    pub slots_updated: usize,
    pub query_count: usize,
    pub candidates_visited: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AuiNodeLookup<'a> {
    document: Option<&'a AuiDocument>,
    slots: Arc<HashMap<String, usize>>,
    control_slots: Arc<Vec<usize>>,
}

impl<'a> AuiNodeLookup<'a> {
    pub fn new(
        document: &'a AuiDocument,
        slots: Arc<HashMap<String, usize>>,
        control_slots: Arc<Vec<usize>>,
    ) -> Self {
        Self {
            document: Some(document),
            slots,
            control_slots,
        }
    }

    pub fn build(document: &'a AuiDocument) -> Self {
        Self::new(
            document,
            Arc::new(node_slots(document)),
            Arc::new(control_slots(document)),
        )
    }

    pub fn get(&self, node_id: &str) -> Option<&'a AuiNode> {
        let document = self.document?;
        self.slots
            .get(node_id)
            .and_then(|&slot| document.nodes.get(slot))
    }

    pub fn control_values(&self) -> impl Iterator<Item = &'a AuiNode> + '_ {
        self.document.into_iter().flat_map(|document| {
            self.control_slots
                .iter()
                .filter_map(|&slot| document.nodes.get(slot))
        })
    }
}

#[derive(Debug, Clone, Default)]
struct AuiCanvasHitGrid {
    cells: HashMap<(i32, i32), Vec<usize>>,
    broad: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
struct AuiHitPlacement {
    canvas_id: String,
    cells: Vec<(i32, i32)>,
    broad: bool,
}

#[derive(Debug, Clone, Default)]
struct AuiHitSpatialIndex {
    canvases: HashMap<String, AuiCanvasHitGrid>,
    placements: Vec<AuiHitPlacement>,
}

impl AuiHitSpatialIndex {
    fn build(layout: &AuiLayoutResult) -> Self {
        let mut index = Self {
            placements: vec![AuiHitPlacement::default(); layout.computed_nodes.len()],
            ..Default::default()
        };
        for slot in 0..layout.computed_nodes.len() {
            index.insert(slot, layout);
        }
        index
    }

    fn update(&mut self, slot: usize, layout: &AuiLayoutResult) {
        self.remove(slot);
        self.insert(slot, layout);
    }

    fn remove(&mut self, slot: usize) {
        let Some(placement) = self.placements.get_mut(slot) else {
            return;
        };
        let Some(canvas) = self.canvases.get_mut(&placement.canvas_id) else {
            *placement = AuiHitPlacement::default();
            return;
        };
        if placement.broad {
            canvas.broad.retain(|candidate| *candidate != slot);
        } else {
            for cell in &placement.cells {
                if let Some(slots) = canvas.cells.get_mut(cell) {
                    slots.retain(|candidate| *candidate != slot);
                    if slots.is_empty() {
                        canvas.cells.remove(cell);
                    }
                }
            }
        }
        *placement = AuiHitPlacement::default();
    }

    fn insert(&mut self, slot: usize, layout: &AuiLayoutResult) {
        let Some(node) = layout.computed_nodes.get(slot) else {
            return;
        };
        if !node.rect.x.is_finite()
            || !node.rect.y.is_finite()
            || !node.rect.width.is_finite()
            || !node.rect.height.is_finite()
            || node.rect.width < 0.0
            || node.rect.height < 0.0
        {
            return;
        }
        let cells = covered_cells(node.presentation_bounds());
        let broad = cells.len() > MAX_CELLS_PER_NODE;
        let canvas = self.canvases.entry(node.canvas_id.clone()).or_default();
        if broad {
            canvas.broad.push(slot);
        } else {
            for cell in &cells {
                canvas.cells.entry(*cell).or_default().push(slot);
            }
        }
        self.placements[slot] = AuiHitPlacement {
            canvas_id: node.canvas_id.clone(),
            cells: if broad { Vec::new() } else { cells },
            broad,
        };
    }
}

fn covered_cells(rect: AuiComputedRect) -> Vec<(i32, i32)> {
    let min_x = cell_coordinate(rect.x);
    let max_x = cell_coordinate(rect.x + rect.width);
    let min_y = cell_coordinate(rect.y);
    let max_y = cell_coordinate(rect.y + rect.height);
    let width = i64::from(max_x) - i64::from(min_x) + 1;
    let height = i64::from(max_y) - i64::from(min_y) + 1;
    if width <= 0 || height <= 0 || width.saturating_mul(height) > MAX_CELLS_PER_NODE as i64 {
        return vec![(min_x, min_y); MAX_CELLS_PER_NODE + 1];
    }
    let mut cells = Vec::with_capacity((width * height) as usize);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            cells.push((x, y));
        }
    }
    cells
}

fn cell_coordinate(value: f32) -> i32 {
    (value / CELL_SIZE)
        .floor()
        .clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn node_slots(document: &AuiDocument) -> HashMap<String, usize> {
    document
        .nodes
        .iter()
        .enumerate()
        .map(|(slot, node)| (node.node_id.clone(), slot))
        .collect()
}

fn control_slots(document: &AuiDocument) -> Vec<usize> {
    document
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(slot, node)| {
            matches!(
                node.kind,
                crate::aui::AuiNodeKind::Toggle | crate::aui::AuiNodeKind::Slider
            )
            .then_some(slot)
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AuiInteractionIndexRuntime {
    generation: u64,
    revision: u64,
    node_slots: Arc<HashMap<String, usize>>,
    control_slots: Arc<Vec<usize>>,
    spatial: AuiHitSpatialIndex,
    candidates: Vec<(usize, AuiPointer)>,
    stats: AuiHitTestIndexStats,
}

// Cache contents and counters are not user-visible interaction state.
impl PartialEq for AuiInteractionIndexRuntime {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl AuiInteractionIndexRuntime {
    pub fn stats(&self) -> AuiHitTestIndexStats {
        self.stats
    }

    pub fn node_lookup<'a>(&self, document: &'a AuiDocument) -> AuiNodeLookup<'a> {
        AuiNodeLookup::new(
            document,
            Arc::clone(&self.node_slots),
            Arc::clone(&self.control_slots),
        )
    }

    pub fn sync(&mut self, document: &AuiDocument, layout: &AuiLayoutResult) {
        let update = &layout.interaction_index_update;
        if update.generation == 0
            || self.generation != update.generation
            || self.spatial.placements.len() != layout.computed_nodes.len()
            || self.node_slots.len() != document.nodes.len()
        {
            self.rebuild(document, layout, update.generation);
            self.revision = update.revision;
            return;
        }
        if self.revision == update.revision {
            return;
        }
        let first_needed = self.revision.saturating_add(1);
        let Some(first_available) = update.history.front().map(|delta| delta.revision) else {
            self.rebuild(document, layout, update.generation);
            self.revision = update.revision;
            return;
        };
        if first_needed < first_available {
            self.rebuild(document, layout, update.generation);
            self.revision = update.revision;
            return;
        }
        let mut expected = first_needed;
        for delta in update
            .history
            .iter()
            .filter(|delta| delta.revision >= first_needed)
        {
            if delta.revision != expected {
                self.rebuild(document, layout, update.generation);
                self.revision = update.revision;
                return;
            }
            for &slot in delta.slots.iter() {
                self.spatial.update(slot, layout);
                self.stats.slots_updated += 1;
            }
            expected = expected.saturating_add(1);
        }
        if expected != update.revision.saturating_add(1) {
            self.rebuild(document, layout, update.generation);
        }
        self.revision = update.revision;
    }

    fn rebuild(&mut self, document: &AuiDocument, layout: &AuiLayoutResult, generation: u64) {
        self.node_slots = Arc::new(node_slots(document));
        self.control_slots = Arc::new(control_slots(document));
        self.spatial = AuiHitSpatialIndex::build(layout);
        self.generation = generation;
        self.stats.full_rebuild_count += 1;
        self.stats.node_table_rebuild_count += 1;
    }

    pub fn candidates(
        &mut self,
        x: f32,
        y: f32,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> &[(usize, AuiPointer)] {
        self.candidates.clear();
        for (canvas_id, canvas) in &self.spatial.canvases {
            let pointer = match presentation {
                Some(presentation) => {
                    let Ok(point) =
                        presentation.target_to_reference(canvas_id, GameViewPoint::new(x, y))
                    else {
                        continue;
                    };
                    AuiPointer::new(point.x, point.y)
                }
                None => AuiPointer::new(x, y),
            };
            let cell = (cell_coordinate(pointer.x), cell_coordinate(pointer.y));
            if let Some(slots) = canvas.cells.get(&cell) {
                self.candidates
                    .extend(slots.iter().map(|&slot| (slot, pointer)));
            }
            self.candidates
                .extend(canvas.broad.iter().map(|&slot| (slot, pointer)));
        }
        self.candidates
            .sort_unstable_by_key(|(slot, _)| std::cmp::Reverse(*slot));
        self.stats.query_count += 1;
        self.stats.candidates_visited += self.candidates.len();
        &self.candidates
    }
}
