//! Retained final geometry shared by drawing and hit testing.
//! One base/result per mounted C2 input; no revision history is retained.
use crate::aui::{AuiClipPolicy, AuiComputedNode, AuiDocument, AuiLayoutResult, AuiResolvedClip};
use crate::aui_layout_runtime::AuiLayoutSnapshot;
use crate::aui_layout_schema::{LayoutVisibility, Overflow};
use project_runtime_sdk::aui_presentation::{Affine2D, NodePresentation, Transform2D};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AuiGeometryUpdateStats {
    /// Only post-layout translations changed; the solver snapshot was clean.
    pub base_reused: bool,
    /// The existing final base was refreshed from a retained solver snapshot.
    pub final_base_reused: bool,
    pub nodes_updated: usize,
    pub paragraphs_updated: usize,
    /// Resolved document nodes examined to establish the source change set.
    pub source_nodes_compared: usize,
    /// Solver snapshot geometry cloned while coordinating C2 publication.
    pub snapshot_nodes_copied: usize,
    /// Authored translation entries examined, including removals, not descendant geometry.
    pub translation_entries_compared: usize,
}

pub(crate) struct AuiGeometryCache {
    base: AuiLayoutResult,
    pub result: Arc<AuiLayoutResult>,
    slots: HashMap<String, Vec<usize>>,
    canvas_slots: HashMap<(String, String), usize>,
    document_slots: Vec<usize>,
    snapshot_slots: Vec<(usize, usize)>,
    parents: Vec<Option<usize>>,
    // Inherited visibility excludes bounds culling: an overflow child can be onscreen
    // while its non-clipping parent is entirely outside the viewport.
    base_subtree_visible: Vec<bool>,
    subtree_visible: Vec<bool>,
    children: Vec<Vec<usize>>,
    clip_owners: Vec<Vec<Option<usize>>>,
    establishes_clip: Vec<bool>,
    own_clips: Vec<Option<AuiResolvedClip>>,
    child_clip_rects: Vec<Option<crate::aui::AuiComputedRect>>,
    translated_child_clip_rects: Vec<Option<crate::aui::AuiComputedRect>>,
    overflows_parent: Vec<bool>,
    scrollbars: Vec<Vec<usize>>,
    offsets: Vec<[f32; 2]>,
    translations: HashMap<String, [f32; 2]>,
    presentations: HashMap<
        String,
        std::collections::BTreeMap<
            project_runtime_sdk::aui::UiProperty,
            project_runtime_sdk::aui::UiValue,
        >,
    >,
    base_presentations: Vec<NodePresentation>,
    matrices: Vec<Affine2D>,
    modulations: Vec<[f32; 4]>,
    pending_base: BTreeSet<usize>,
    pub translation_entries_compared: usize,
}

impl AuiGeometryCache {
    pub fn new(
        document: &AuiDocument,
        snapshots: &[&AuiLayoutSnapshot],
        base: AuiLayoutResult,
    ) -> Self {
        let canvas_slots: HashMap<_, _> = base
            .computed_nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| ((node.canvas_id.clone(), node.node_id.clone()), slot))
            .collect();
        let mut slots = HashMap::<String, Vec<usize>>::new();
        for (slot, node) in base.computed_nodes.iter().enumerate() {
            slots.entry(node.node_id.clone()).or_default().push(slot);
        }
        let document_by_id: HashMap<_, _> = document
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.node_id.as_str(), slot))
            .collect();
        let snapshot_by_id: HashMap<_, _> = snapshots
            .iter()
            .enumerate()
            .flat_map(|(snapshot_slot, snapshot)| {
                snapshot
                    .nodes
                    .iter()
                    .enumerate()
                    .map(move |(node_slot, node)| {
                        (
                            (snapshot.canvas_id.as_str(), node.node_id.as_str()),
                            (snapshot_slot, node_slot),
                        )
                    })
            })
            .collect();
        let document_slots: Vec<_> = base
            .computed_nodes
            .iter()
            .map(|node| document_by_id[node.node_id.as_str()])
            .collect();
        let snapshot_slots: Vec<_> = base
            .computed_nodes
            .iter()
            .map(|node| {
                snapshot_by_id
                    .get(&(node.canvas_id.as_str(), node.node_id.as_str()))
                    .copied()
                    .unwrap_or_default()
            })
            .collect();
        let parents: Vec<_> = base
            .computed_nodes
            .iter()
            .zip(&document_slots)
            .map(|(computed, &document_slot)| {
                document.nodes[document_slot]
                    .parent
                    .as_ref()
                    .and_then(|parent| {
                        canvas_slots.get(&(computed.canvas_id.clone(), parent.clone()))
                    })
                    .copied()
            })
            .collect();
        let mut children = vec![Vec::new(); parents.len()];
        let mut base_subtree_visible = vec![true; parents.len()];
        for (slot, parent) in parents.iter().enumerate() {
            base_subtree_visible[slot] = base.computed_nodes[slot].local_visible
                && parent.is_none_or(|parent| base_subtree_visible[parent]);
            if let Some(parent) = parent {
                debug_assert!(*parent < slot, "layout must be parent-before-child");
                children[*parent].push(slot);
            }
        }
        let clip_owners = base
            .computed_nodes
            .iter()
            .map(|node| {
                node.effective_clips
                    .iter()
                    .map(|clip| {
                        canvas_slots
                            .get(&(node.canvas_id.clone(), clip.node_id.clone()))
                            .copied()
                    })
                    .collect()
            })
            .collect();
        let establishes_clip: Vec<_> = document_slots
            .iter()
            .map(|&slot| establishes_clip(&document.nodes[slot]))
            .collect();
        let own_clips = base
            .computed_nodes
            .iter()
            .zip(&document_slots)
            .zip(&establishes_clip)
            .map(|((computed, &document_slot), &establishes)| {
                establishes.then(|| resolved_clip(&document.nodes[document_slot], computed.rect))
            })
            .collect();
        let mut child_clip_rects = vec![None; base.computed_nodes.len()];
        for slot in 0..base.computed_nodes.len() {
            let incoming = parents[slot].and_then(|parent| child_clip_rects[parent]);
            child_clip_rects[slot] = child_clip_rect(
                incoming,
                !base.computed_nodes[slot].effective_clips.is_empty(),
                establishes_clip[slot],
                base.computed_nodes[slot].rect,
            );
        }
        let overflows_parent = snapshot_slots
            .iter()
            .map(|&(snapshot, node)| {
                snapshots
                    .get(snapshot)
                    .and_then(|s| s.nodes.get(node))
                    .is_some_and(|n| n.overflows_parent)
            })
            .collect();
        let mut scrollbars = vec![Vec::new(); parents.len()];
        for (index, scrollbar) in base.scrollbar_metrics.iter().enumerate() {
            if let Some(owners) = slots.get(&scrollbar.scroll_node_id) {
                for &slot in owners {
                    scrollbars[slot].push(index);
                }
            }
        }
        let base_presentations: Vec<_> = document_slots
            .iter()
            .map(|&slot| document.nodes[slot].presentation.unwrap_or_default())
            .collect();
        let initial_pending = (0..parents.len()).collect();
        Self {
            matrices: vec![Affine2D::IDENTITY; parents.len()],
            modulations: vec![[1.0; 4]; parents.len()],
            base_presentations,
            presentations: HashMap::new(),
            offsets: vec![[0.; 2]; parents.len()],
            result: Arc::new(base.clone()),
            base,
            slots,
            canvas_slots,
            document_slots,
            snapshot_slots,
            parents,
            subtree_visible: base_subtree_visible.clone(),
            base_subtree_visible,
            children,
            clip_owners,
            establishes_clip,
            own_clips,
            translated_child_clip_rects: child_clip_rects.clone(),
            child_clip_rects,
            overflows_parent,
            scrollbars,
            translations: HashMap::new(),
            pending_base: initial_pending,
            translation_entries_compared: 0,
        }
    }

    /// Refresh final C2 geometry from changed retained-snapshot nodes. Descendants are added only
    /// when inherited visibility or the effective clip chain can have changed.
    pub fn refresh_base(
        &mut self,
        document: &AuiDocument,
        snapshots: &[&AuiLayoutSnapshot],
        changed_node_ids: &HashSet<String>,
        frame: u64,
    ) -> Vec<usize> {
        let mut pending = changed_node_ids
            .iter()
            .filter_map(|id| self.slots.get(id))
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut affected = Vec::new();
        while let Some(slot) = pending.pop_first() {
            let document_node = &document.nodes[self.document_slots[slot]];
            let (snapshot_slot, geometry_slot) = self.snapshot_slots[slot];
            let geometry = &snapshots[snapshot_slot].nodes[geometry_slot];
            let old = self.base.computed_nodes[slot].clone();
            let old_establishes = self.establishes_clip[slot];
            let old_own_clip = self.own_clips[slot].clone();
            let old_child_clip_rect = self.child_clip_rects[slot];
            let effective_clips = self.parents[slot]
                .map(|parent| {
                    let mut clips = self.base.computed_nodes[parent].effective_clips.clone();
                    if let Some(clip) = self.own_clips[parent].clone() {
                        clips.push(clip);
                    }
                    clips
                })
                .unwrap_or_default();
            let clipped = !effective_clips.is_empty();
            let incoming_clip_rect =
                self.parents[slot].and_then(|parent| self.child_clip_rects[parent]);
            let effective_clip_rect = if clipped {
                incoming_clip_rect.and_then(|rect| rect.intersection(geometry.rect))
            } else {
                None
            };
            let local_visible =
                document_node.visible && geometry.visibility == LayoutVisibility::Visible;
            let effective_visible = local_visible
                && self.parents[slot]
                    .map(|parent| self.base_subtree_visible[parent])
                    .unwrap_or(true)
                && (!clipped || effective_clip_rect.is_some());
            let subtree_visible = local_visible
                && self.parents[slot].is_none_or(|parent| self.base_subtree_visible[parent]);
            let node = AuiComputedNode {
                presentation: None,
                canvas_id: old.canvas_id.clone(),
                composition_stage: old.composition_stage,
                node_id: document_node.node_id.clone(),
                kind: document_node.kind,
                rect: geometry.rect,
                effective_clip_rect,
                clipped_by_node: effective_clips.last().map(|clip| clip.node_id.clone()),
                effective_clips,
                tree_order: old.tree_order,
                local_visible,
                effective_visible,
            };
            let establishes = establishes_clip(document_node);
            let own_clip = establishes.then(|| resolved_clip(document_node, geometry.rect));
            let child_clip_rect =
                child_clip_rect(incoming_clip_rect, clipped, establishes, geometry.rect);
            let inherited_changed = old.rect != node.rect
                || self.base_subtree_visible[slot] != subtree_visible
                || self.base_presentations[slot] != document_node.presentation.unwrap_or_default()
                || old.effective_visible != node.effective_visible
                || old.effective_clips != node.effective_clips
                || old_establishes != establishes
                || old_own_clip != own_clip
                || old_child_clip_rect != child_clip_rect;

            update_report_node(&mut self.base, &old, &node);
            self.base.computed_nodes[slot] = node;
            self.base_subtree_visible[slot] = subtree_visible;
            self.base_presentations[slot] = document_node.presentation.unwrap_or_default();
            self.establishes_clip[slot] = establishes;
            self.own_clips[slot] = own_clip;
            self.child_clip_rects[slot] = child_clip_rect;
            self.clip_owners[slot] = self.base.computed_nodes[slot]
                .effective_clips
                .iter()
                .map(|clip| {
                    self.canvas_slots
                        .get(&(
                            self.base.computed_nodes[slot].canvas_id.clone(),
                            clip.node_id.clone(),
                        ))
                        .copied()
                })
                .collect();
            let overflows = geometry.overflows_parent;
            if self.overflows_parent[slot] != overflows {
                if overflows {
                    self.base.report.overflow_count += 1;
                } else {
                    self.base.report.overflow_count -= 1;
                }
                self.overflows_parent[slot] = overflows;
            }
            if old_establishes != establishes {
                if establishes {
                    self.base.report.clip_root_count += 1;
                } else {
                    self.base.report.clip_root_count -= 1;
                }
            }
            if inherited_changed {
                pending.extend(self.children[slot].iter().copied());
            }
            self.pending_base.insert(slot);
            affected.push(slot);
        }
        self.base.report.frame = frame;
        affected
    }

    pub fn update_presentations(
        &mut self,
        translations: &HashMap<String, [f32; 2]>,
        presentations: &HashMap<
            String,
            std::collections::BTreeMap<
                project_runtime_sdk::aui::UiProperty,
                project_runtime_sdk::aui::UiValue,
            >,
        >,
        changes: Option<&HashSet<String>>,
        frame: u64,
    ) -> Vec<usize> {
        // refresh_base already computed the exact inherited-geometry closure. Only authored
        // translations need unconditional descendant propagation here.
        let mut affected = std::mem::take(&mut self.pending_base);
        let mut pending = Vec::<usize>::new();
        let ids: HashSet<String> = match changes {
            Some(changes) => changes.clone(),
            None => translations
                .keys()
                .chain(self.translations.keys())
                .chain(presentations.keys())
                .chain(self.presentations.keys())
                .cloned()
                .collect(),
        };
        self.translation_entries_compared = ids.len();
        for id in &ids {
            if self.translations.get(id).copied().unwrap_or_default()
                != translations.get(id).copied().unwrap_or_default()
                || self.presentations.get(id) != presentations.get(id)
            {
                if let Some(slots) = self.slots.get(id) {
                    pending.extend(slots.iter().copied());
                }
            }
        }
        let mut translated = BTreeSet::new();
        while let Some(slot) = pending.pop() {
            if translated.insert(slot) {
                pending.extend(&self.children[slot]);
            }
        }
        affected.extend(translated);
        let result = Arc::make_mut(&mut self.result);
        for &slot in &affected {
            let old = result.computed_nodes[slot].clone();
            let mut node = self.base.computed_nodes[slot].clone();
            let inherited = self.parents[slot]
                .map(|parent| self.offsets[parent])
                .unwrap_or_default();
            let mut properties = self.base_presentations[slot];
            if let Some(overrides) = presentations.get(&node.node_id) {
                for (&property, value) in overrides {
                    properties.apply(property, value);
                }
            }
            let local = translations
                .get(&node.node_id)
                .copied()
                .unwrap_or(properties.transform.translation);
            let mut offset = [inherited[0] + local[0], inherited[1] + local[1]];
            let translated = [node.rect.x + offset[0], node.rect.y + offset[1]];
            let rect_representable = offset
                .iter()
                .chain(translated.iter())
                .all(|v| v.is_finite());
            if !rect_representable {
                offset = [0.; 2];
            }
            self.offsets[slot] = offset;
            node.rect.x += offset[0];
            node.rect.y += offset[1];
            let parent = self.parents[slot];
            let base_rect = self.base.computed_nodes[slot].rect;
            let parent_rect = parent
                .map(|p| self.base.computed_nodes[p].rect)
                .unwrap_or_default();
            properties.transform.translation = local;
            let matrix = properties.validate().and_then(|()| {
                properties
                    .transform
                    .local_matrix(
                        [base_rect.x - parent_rect.x, base_rect.y - parent_rect.y],
                        [base_rect.width, base_rect.height],
                    )
                    .and_then(|local| {
                        parent
                            .map_or(Affine2D::IDENTITY, |p| self.matrices[p])
                            .compose(local)
                    })
            });
            let matrix_representable = matrix.is_ok();
            let matrix = match matrix {
                Ok(matrix) => matrix,
                Err(error) => {
                    result
                        .diagnostics
                        .push(crate::aui::AuiValidationItem::error(
                            "aui_presentation_invalid",
                            Some(node.node_id.clone()),
                            None,
                            format!("{error}"),
                            "Use representable finite presentation values.",
                        ));
                    // A finite degenerate matrix keeps descendants and rendering safe.
                    Affine2D::from_coefficients([0.0; 6]).expect("finite zero")
                }
            };
            self.matrices[slot] = matrix;
            let inherited_color = parent.map_or([1.0; 4], |p| self.modulations[p]);
            let mut modulation = std::array::from_fn(|i| inherited_color[i] * properties.tint[i]);
            modulation[3] *= properties.opacity;
            self.modulations[slot] = modulation;
            let bounds = transformed_bounds(base_rect.width, base_rect.height, matrix);
            let vertex_transform = matrix.compose(
                Transform2D {
                    translation: [-node.rect.x, -node.rect.y],
                    ..Default::default()
                }
                .local_matrix([0.0; 2], [0.0; 2])
                .expect("finite origin"),
            );
            node.presentation =
                vertex_transform
                    .ok()
                    .map(|vertex| crate::aui::AuiPresentationGeometry {
                        local_to_canvas: matrix.coefficients(),
                        vertex_transform: vertex.coefficients(),
                        bounds,
                        modulation,
                    });
            let representable = rect_representable
                && matrix_representable
                && node.presentation.is_some()
                && [bounds.x, bounds.y, bounds.width, bounds.height]
                    .iter()
                    .all(|v| v.is_finite())
                && [
                    [0., 0.],
                    [base_rect.width, 0.],
                    [0., base_rect.height],
                    [base_rect.width, base_rect.height],
                ]
                .iter()
                .all(|p| matrix.transform_point(*p).is_ok());
            if !representable {
                node.presentation = Some(crate::aui::AuiPresentationGeometry {
                    local_to_canvas: [0.; 6],
                    vertex_transform: [0.; 6],
                    bounds: Default::default(),
                    modulation,
                });
                result
                    .diagnostics
                    .push(crate::aui::AuiValidationItem::error(
                        "aui_presentation_invalid",
                        Some(node.node_id.clone()),
                        None,
                        "Final vertices or bounds exceed finite f32 representation.",
                        "Reduce the combined transform magnitude.",
                    ));
            }
            for (clip, owner) in node.effective_clips.iter_mut().zip(&self.clip_owners[slot]) {
                let offset = owner.map(|owner| self.offsets[owner]).unwrap_or_default();
                clip.rect.x += offset[0];
                clip.rect.y += offset[1];
                clip.transform = owner.and_then(|owner| {
                    result.computed_nodes[owner]
                        .presentation
                        .map(|p| p.vertex_transform)
                });
            }
            let clipped = !node.effective_clips.is_empty();
            let incoming_clip_rect =
                self.parents[slot].and_then(|parent| self.translated_child_clip_rects[parent]);
            node.effective_clip_rect = if clipped {
                incoming_clip_rect.and_then(|rect| rect.intersection(bounds))
            } else {
                None
            };
            self.subtree_visible[slot] = node.local_visible
                && representable
                && self.parents[slot]
                    .map(|parent| self.subtree_visible[parent])
                    .unwrap_or(true);
            node.effective_visible =
                self.subtree_visible[slot] && (!clipped || node.effective_clip_rect.is_some());
            self.translated_child_clip_rects[slot] = child_clip_rect(
                incoming_clip_rect,
                clipped,
                self.establishes_clip[slot],
                bounds,
            );
            if node.effective_clips.len() > 8 {
                self.subtree_visible[slot] = false;
                node.effective_visible = false;
                result
                    .diagnostics
                    .push(crate::aui::AuiValidationItem::error(
                        "aui_clip_depth_exceeded",
                        Some(node.node_id.clone()),
                        None,
                        "Exact clip depth exceeds 8.",
                        "Split or reduce clipping ancestors.",
                    ));
            }
            update_report_node(result, &old, &node);
            result.computed_nodes[slot] = node;
            for &index in &self.scrollbars[slot] {
                let mut scrollbar = self.base.scrollbar_metrics[index].clone();
                scrollbar.track_rect.x += offset[0];
                scrollbar.track_rect.y += offset[1];
                scrollbar.thumb_rect.x += offset[0];
                scrollbar.thumb_rect.y += offset[1];
                result.scrollbar_metrics[index] = scrollbar;
            }
        }
        for id in ids {
            if let Some(presentation) = presentations.get(&id) {
                self.presentations.insert(id.clone(), presentation.clone());
            } else {
                self.presentations.remove(&id);
            }
            if let Some(offset) = translations.get(&id) {
                self.translations.insert(id, *offset);
            } else {
                self.translations.remove(&id);
            }
        }
        result.report.frame = frame;
        result.report.overflow_count = self.base.report.overflow_count;
        result.report.clip_root_count = self.base.report.clip_root_count;
        affected.into_iter().collect()
    }

    pub fn set_interaction_index_update(
        &mut self,
        update: crate::aui_hit_test_index::AuiInteractionIndexUpdate,
    ) {
        Arc::make_mut(&mut self.result).interaction_index_update = update;
    }
}

fn establishes_clip(node: &crate::aui::AuiNode) -> bool {
    matches!(node.overflow, Some(Overflow::Clip | Overflow::Scroll))
        || matches!(
            node.clip_policy,
            AuiClipPolicy::Rect | AuiClipPolicy::Rounded
        )
}

pub(crate) fn transformed_bounds(
    width: f32,
    height: f32,
    matrix: Affine2D,
) -> crate::aui::AuiComputedRect {
    let points = [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]]
        .map(|p| matrix.transform_point(p));
    if points.iter().any(Result::is_err) {
        return crate::aui::AuiComputedRect::default();
    }
    let points = points.map(Result::unwrap);
    let x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    crate::aui::AuiComputedRect {
        x,
        y,
        width: max_x - x,
        height: max_y - y,
    }
}

fn resolved_clip(node: &crate::aui::AuiNode, rect: crate::aui::AuiComputedRect) -> AuiResolvedClip {
    AuiResolvedClip {
        transform: None,
        node_id: node.node_id.clone(),
        rect,
        radius: if node.clip_policy == AuiClipPolicy::Rounded {
            node.clip_radius
                .or_else(|| node.resolved_style.as_ref().map(|style| style.paint.radius))
                .unwrap_or_default()
        } else {
            0.0
        },
    }
}

fn child_clip_rect(
    incoming: Option<crate::aui::AuiComputedRect>,
    inherited_clipped: bool,
    establishes: bool,
    rect: crate::aui::AuiComputedRect,
) -> Option<crate::aui::AuiComputedRect> {
    if establishes {
        match incoming {
            Some(parent) => parent.intersection(rect),
            None if inherited_clipped => None,
            None => Some(rect),
        }
    } else {
        incoming
    }
}

fn update_report_node(result: &mut AuiLayoutResult, old: &AuiComputedNode, new: &AuiComputedNode) {
    result.report.visible_node_count = result.report.visible_node_count
        - usize::from(old.effective_visible)
        + usize::from(new.effective_visible);
    result.report.clipped_node_count = result.report.clipped_node_count
        - usize::from(old.clipped_by_node.is_some() && old.effective_clip_rect.is_none())
        + usize::from(new.clipped_by_node.is_some() && new.effective_clip_rect.is_none());
    result.report.effective_clip_node_count = result.report.effective_clip_node_count
        - usize::from(old.effective_clip_rect.is_some())
        + usize::from(new.effective_clip_rect.is_some());
}
