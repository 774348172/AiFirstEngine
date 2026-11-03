//! Shared graph/native attachment checks; no new resource registry.
use super::*;
use std::collections::BTreeMap;
#[derive(Default)]
#[doc(hidden)]
pub struct DepthAttachmentValidation<'a> {
    views: BTreeMap<&'a str, Vec<&'a ExecutableRenderView>>,
}
impl<'a> DepthAttachmentValidation<'a> {
    #[doc(hidden)]
    pub fn insert(
        &mut self,
        id: &'a str,
        view: &'a ExecutableRenderView,
    ) -> Result<(), ContractError> {
        let previous = self.views.entry(id).or_default();
        if !view.is_shadow_map() {
            if !previous.is_empty() {
                return Err(invalid("geometry.shared_view_depth"));
            }
        } else {
            if previous.is_empty() {
                if view.depth_clear.map(OrderedF32::to_f32) != Some(1.) {
                    return Err(invalid("shadow.atlas_uninitialized"));
                }
            } else if view.depth_clear.is_some() {
                return Err(invalid("shadow.atlas_reclear"));
            }
            let [x, y, w, h] = view.viewport;
            for old in previous.iter() {
                let [a, b, c, d] = old.viewport;
                if !old.is_shadow_map()
                    || old.target_extent != view.target_extent
                    || (u64::from(x) < u64::from(a) + u64::from(c)
                        && u64::from(a) < u64::from(x) + u64::from(w)
                        && u64::from(y) < u64::from(b) + u64::from(d)
                        && u64::from(b) < u64::from(y) + u64::from(h))
                {
                    return Err(invalid("shadow.atlas_overlap_or_extent"));
                }
            }
        }
        previous.push(view);
        Ok(())
    }
}

/// One depth projection, in the receiver's world-relative coordinate system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShadowProjection {
    pub atlas: String,
    /// 0 directional cascade, 1 spot. Point shadows are deliberately unsupported.
    pub slot: u32,
    pub light_index: u32,
    pub matrix: [OrderedF32; 16],
    pub viewport: [u32; 4],
    pub extent: [u32; 2],
    pub depth_range: [OrderedF32; 2],
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShadowMaps {
    view: Arc<ExecutableRenderView>,
    projections: Vec<ShadowProjection>,
}
impl ShadowMaps {
    pub fn build(
        view: &ExecutableRenderView,
        lights: &ClusteredLights,
        projections: Vec<ShadowProjection>,
    ) -> Result<Arc<Self>, ContractError> {
        let maps = Self {
            view: Arc::new(view.clone()),
            projections,
        };
        maps.validate(view, lights)?;
        Ok(Arc::new(maps))
    }
    pub fn projections(&self) -> &[ShadowProjection] {
        &self.projections
    }
    #[doc(hidden)]
    pub fn texture_mask(&self) -> u8 {
        self.projections
            .iter()
            .fold(0, |mask, p| mask | (1 << p.slot))
    }
    #[doc(hidden)]
    pub fn atlases(&self) -> impl Iterator<Item = &str> {
        self.projections.iter().map(|p| p.atlas.as_str())
    }
    #[doc(hidden)]
    pub fn words(&self) -> Vec<u32> {
        let mut words = vec![self.projections.len() as u32, 0, 0, 0];
        words.extend(self.view.view_matrix.map(|v| v.to_f32().to_bits()));
        for p in &self.projections {
            words.extend(p.matrix.map(|v| v.to_f32().to_bits()));
            let [x, y, w, h] = p.viewport;
            let [a, b] = p.extent;
            words.extend(
                [
                    x as f32 / a as f32,
                    y as f32 / b as f32,
                    w as f32 / a as f32,
                    h as f32 / b as f32,
                ]
                .map(f32::to_bits),
            );
            words.extend([
                p.light_index,
                p.slot,
                p.depth_range[0].to_f32().to_bits(),
                p.depth_range[1].to_f32().to_bits(),
            ]);
        }
        words
    }
    pub(crate) fn validate(
        &self,
        view: &ExecutableRenderView,
        lights: &ClusteredLights,
    ) -> Result<(), ContractError> {
        if self.view.as_ref() != view
            || self.projections.is_empty()
            || self.projections.len() > 12
            || view.is_shadow_map()
        {
            return Err(invalid("shadow.receiver"));
        }
        let mut atlas: [Option<&str>; 2] = [None, None];
        let mut directional = 0;
        let mut spots = std::collections::BTreeSet::new();
        for p in &self.projections {
            if p.slot > 1
                || p.light_index as usize >= lights.light_count()
                || p.atlas.trim().is_empty()
                || p.extent.contains(&0)
                || p.matrix.iter().any(|v| !v.to_f32().is_finite())
                || p.depth_range.iter().any(|v| !v.to_f32().is_finite())
                || p.depth_range[0].to_f32() >= p.depth_range[1].to_f32()
            {
                return Err(invalid("shadow.projection"));
            }
            let [x, y, w, h] = p.viewport;
            if w < 2
                || h < 2
                || x.checked_add(w).is_none_or(|v| v > p.extent[0])
                || y.checked_add(h).is_none_or(|v| v > p.extent[1])
            {
                return Err(invalid("shadow.tile"));
            }
            let kind = f32::from_bits(lights.words()[20 + p.light_index as usize * 16 + 3]);
            if (p.slot == 0 && kind != 0.) || (p.slot == 1 && kind != 2.) {
                return Err(invalid("shadow.light_kind"));
            }
            if atlas[p.slot as usize].is_some_and(|old| old != p.atlas) {
                return Err(invalid("shadow.atlas_count"));
            }
            atlas[p.slot as usize] = Some(&p.atlas);
            if p.slot == 0 {
                directional += 1;
                if directional > 4 {
                    return Err(invalid("shadow.cascade_capacity"));
                }
            } else if !spots.insert(p.light_index) || spots.len() > 8 {
                return Err(invalid("shadow.spot_capacity"));
            }
        }
        if atlas[0].is_some() && atlas[0] == atlas[1] {
            return Err(invalid("shadow.atlas_slots"));
        }
        Ok(())
    }
    #[doc(hidden)]
    pub fn observe_memory(&self, s: &mut crate::resource_memory::ResourceMemorySample, role: &str) {
        s.cpu_arc(&self.view, "snapshots", &[role]);
        s.cpu_vec(&self.projections, "snapshots", &[role]);
        for p in &self.projections {
            s.cpu_string(&p.atlas, "snapshots", &[role]);
        }
    }
}

impl ShadowMaps {
    pub(crate) fn validate_graph(
        &self,
        graph: &crate::render_graph::RenderGraph,
        pass: &crate::render_graph::RenderPass,
        index: usize,
    ) -> Result<(), ContractError> {
        use crate::render_graph::*;
        for p in &self.projections {
            let r = graph
                .resources
                .iter()
                .find(|r| r.resource_id == p.atlas)
                .ok_or_else(|| invalid("shadow.resource_missing"))?;
            let shape = graph
                .contracts
                .resources
                .get(&GraphResourceId(p.atlas.clone()))
                .cloned()
                .unwrap_or_default();
            if r.format != RenderResourceFormat::Depth32Float
                || !r.usage.contains(&RenderResourceUsage::Sampled)
                || shape.samples != 1
                || shape.mip_levels != 1
                || shape.layers != 1
                || [r.size.width, r.size.height] != p.extent
                || !pass.reads.contains(&p.atlas)
            {
                return Err(invalid("shadow.sample_descriptor"));
            }
            let mut found = false;
            for (i, writer) in graph.passes.iter().enumerate() {
                if writer.depth_target.as_deref() != Some(&p.atlas) {
                    continue;
                }
                if i >= index {
                    return Err(invalid("shadow.writer_after_receiver"));
                }
                for c in &writer.commands {
                    if let RenderPassCommand::BeginView { target, view, .. } = c {
                        if target == &p.atlas && p.matches_writer(view) {
                            found = true;
                        }
                    }
                }
            }
            if !found {
                return Err(invalid("shadow.writer_mismatch"));
            }
        }
        Ok(())
    }
}
impl ShadowProjection {
    #[doc(hidden)]
    pub fn matches_writer(&self, view: &ExecutableRenderView) -> bool {
        view.is_shadow_map()
            && view.viewport == self.viewport
            && view.target_extent == self.extent
            && view.view_projection().map(OrderedF32::from) == self.matrix
    }
}

#[cfg_attr(test, path = "shadow/planning.rs")]
mod planning;
#[cfg(test)]
pub(super) use planning::append_scene_lighting;
pub(super) use planning::build_scene_lighting;
#[cfg(test)]
pub(super) use planning::tests::receiver_scene;
