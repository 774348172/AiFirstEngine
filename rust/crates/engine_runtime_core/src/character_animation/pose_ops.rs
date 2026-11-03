//! Authored IK and explicit retarget bindings consumed by the existing character stage.
use super::*;
use crate::spatial_contract::SpatialAssetRef;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRetargetBinding3D {
    pub source: SpatialAssetRef,
    pub mapping: RetargetMap3D,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PoseIk3D {
    pub root: String,
    pub middle: String,
    pub end: String,
    /// Target and pole in the skeleton's parent frame, metres.
    pub target: [f32; 3],
    pub pole: [f32; 3],
    pub weight: f32,
}
impl PoseIk3D {
    pub fn validate(&self, asset: &CharacterAsset) -> ContractResult<[usize; 3]> {
        if !self.weight.is_finite()
            || !(0.0..=1.0).contains(&self.weight)
            || self.target.iter().chain(&self.pole).any(|v| !v.is_finite())
        {
            return Err(invalid("/ik", "Invalid weight, target or pole."));
        }
        let keys = [&self.root, &self.middle, &self.end];
        let mut indices = [0; 3];
        for (i, key) in keys.iter().enumerate() {
            indices[i] = asset
                .joints
                .iter()
                .position(|j| &j.key == *key)
                .ok_or_else(|| invalid("/ik/joint", "IK joint does not exist."))?;
        }
        if asset.joints[indices[1]].parent != Some(indices[0] as u16)
            || asset.joints[indices[2]].parent != Some(indices[1] as u16)
        {
            return Err(invalid(
                "/ik/hierarchy",
                "IK requires three consecutive joints.",
            ));
        }
        Ok(indices)
    }
    pub fn apply(
        &self,
        asset: &CharacterAsset,
        pose: &mut SampledCharacterPose,
    ) -> ContractResult<IkReach> {
        let [root, middle, end] = self.validate(asset)?;
        let globals = asset.global_matrices(pose)?;
        let matrices = globals
            .iter()
            .map(Mat4::from_cols_array)
            .collect::<Vec<_>>();
        let r = matrices[root].transform_point3(Vec3::ZERO);
        let j = matrices[middle].transform_point3(Vec3::ZERO);
        let e = matrices[end].transform_point3(Vec3::ZERO);
        // Rotation-only IK preserves bone lengths. Sheared/nonuniform chains require a
        // different declared solver, rather than silently changing scales or translations.
        for i in [root, middle, end] {
            let (scale, rotation, translation) = matrices[i].to_scale_rotation_translation();
            let rebuilt = Mat4::from_scale_rotation_translation(scale, rotation, translation);
            if (scale.x - scale.y).abs() > 1e-4
                || (scale.x - scale.z).abs() > 1e-4
                || scale.min_element() <= 0.
                || rebuilt
                    .to_cols_array()
                    .iter()
                    .zip(globals[i])
                    .any(|(a, b)| (a - b).abs() > 1e-4)
            {
                return Err(invalid(
                    "/ik/scale",
                    "Rotation IK requires positive uniform-scale, shear-free joints.",
                ));
            }
        }
        let (wanted_j, wanted_e, reach) = two_bone_ik(
            r.to_array(),
            j.to_array(),
            e.to_array(),
            self.target,
            self.pole,
        )?;
        let root_world = matrices[root].to_scale_rotation_translation().1;
        let rotation = Quat::from_rotation_arc(
            (j - r).normalize(),
            (Vec3::from_array(wanted_j) - r).normalize(),
        ) * root_world;
        let parent = asset.joints[root].parent.map_or(Quat::IDENTITY, |p| {
            matrices[p as usize].to_scale_rotation_translation().1
        });
        let mut candidate = pose.clone();
        candidate.joints[root].rotation = (parent.inverse() * rotation).normalize().to_array();
        let second = asset.global_matrices(&candidate)?;
        let jm = Mat4::from_cols_array(&second[middle]);
        let em = Mat4::from_cols_array(&second[end]);
        let new_j = jm.transform_point3(Vec3::ZERO);
        let new_e = em.transform_point3(Vec3::ZERO);
        let mid_world = Quat::from_rotation_arc(
            (new_e - new_j).normalize(),
            (Vec3::from_array(wanted_e) - new_j).normalize(),
        ) * jm.to_scale_rotation_translation().1;
        let root_world = Mat4::from_cols_array(&second[root])
            .to_scale_rotation_translation()
            .1;
        candidate.joints[middle].rotation =
            (root_world.inverse() * mid_world).normalize().to_array();
        blend_pose(
            pose,
            &candidate,
            self.weight,
            &[root as u16, middle as u16],
            None,
        )?;
        Ok(reach)
    }
}
