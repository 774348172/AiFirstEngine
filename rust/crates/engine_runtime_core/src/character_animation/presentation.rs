use super::*;
use crate::{ids::EntityId, world::World};
use std::sync::Arc;
impl World {
    /// Presentation-only interpolation. Fixed clocks, events and gameplay pose
    /// remain untouched, including frames with zero simulation ticks.
    pub(crate) fn prepare_character_presentation(
        &mut self,
        alpha: f32,
    ) -> ContractResult<Vec<EntityId>> {
        self.character_presented_skins
            .retain(|id, _| self.character_skins.contains_key(id));
        for frame in self.character_poses.values() {
            let mut pose = frame.previous_visual.clone();
            blend_pose(&mut pose, &frame.visual, alpha, &[], None)?;
            for (index, skin) in frame.asset.skins.iter().enumerate() {
                let target: EntityId = frame.joints[skin.mesh_joint as usize].clone();
                let matrices = frame.asset.skin_matrices(index, &pose)?;
                let weights = pose
                    .morph_weights
                    .iter()
                    .find(|(j, _)| *j == skin.mesh_joint)
                    .map(|(_, w)| w.as_slice())
                    .unwrap_or(&skin.morph_weights);
                let mut deformation = SkinDeformation3D::new(
                    frame.skin_sources[index].clone(),
                    &matrices,
                    &matrices,
                    weights,
                    weights,
                    frame.history_reset,
                )?;
                if !frame.history_reset {
                    if let Some(old) = self.character_presented_skins.get(&target) {
                        deformation.previous_matrices = old.current_matrices.clone();
                        deformation.previous_weights = old.current_weights.clone();
                    }
                }
                let deformation = Arc::new(deformation);
                self.character_skins
                    .insert(target.clone(), deformation.clone());
                self.character_presented_skins.insert(target, deformation);
            }
        }
        let targets = self
            .character_presented_skins
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for target in &targets {
            self.mark_render_payload_dirty(target.clone());
        }
        Ok(targets)
    }
}
