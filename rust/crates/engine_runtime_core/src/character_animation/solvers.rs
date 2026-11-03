//! Explicit, bounded pose operations. Visual callers never write World simulation.
use super::*;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IkReach {
    Reached,
    Clamped,
}
/// Two segment IK in one declared coordinate frame. Preserves both segment lengths.
pub fn two_bone_ik(
    root: [f32; 3],
    joint: [f32; 3],
    end: [f32; 3],
    target: [f32; 3],
    pole: [f32; 3],
) -> ContractResult<([f32; 3], [f32; 3], IkReach)> {
    let [r, j, e, t, p] = [root, joint, end, target, pole].map(Vec3::from_array);
    let a = (j - r).length();
    let b = (e - j).length();
    let delta = t - r;
    let distance = delta.length();
    if [r, j, e, t, p].iter().any(|v| !v.is_finite()) || a < 1e-5 || b < 1e-5 || distance < 1e-6 {
        return Err(invalid("/ik", "Degenerate IK chain or target."));
    }
    let direction = delta / distance;
    let pole_delta = p - r;
    let side = pole_delta - direction * pole_delta.dot(direction);
    if side.length_squared() < 1e-10 {
        return Err(invalid(
            "/ik/pole",
            "Pole must define a plane with the target.",
        ));
    }
    let d = distance.clamp((a - b).abs() + 1e-5, a + b - 1e-5);
    let x = (a * a - b * b + d * d) / (2. * d);
    let height = (a * a - x * x).max(0.).sqrt();
    Ok((
        (r + direction * x + side.normalize() * height).to_array(),
        (if d == distance { t } else { r + direction * d }).to_array(),
        if (d - distance).abs() < 1e-4 {
            IkReach::Reached
        } else {
            IkReach::Clamped
        },
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetargetJoint3D {
    pub source: String,
    pub target: String,
    pub translation_scale: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetargetMap3D {
    pub source_asset: String,
    pub target_asset: String,
    pub joints: Vec<RetargetJoint3D>,
}
impl RetargetMap3D {
    /// Bind-relative local deltas. Unmapped destination joints retain their bind pose.
    pub fn apply(
        &self,
        source: &CharacterAsset,
        target: &CharacterAsset,
        pose: &SampledCharacterPose,
    ) -> ContractResult<SampledCharacterPose> {
        source.validate()?;
        target.validate()?;
        if self.source_asset != source.asset_id
            || self.target_asset != target.asset_id
            || self.joints.is_empty()
            || pose.joints.len() != source.joints.len()
        {
            return Err(invalid("/retarget", "Wrong rig identity or pose size."));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut result = SampledCharacterPose {
            joints: target.joints.iter().map(|j| j.bind.clone()).collect(),
            morph_weights: target
                .skins
                .iter()
                .map(|s| (s.mesh_joint, s.morph_weights.clone()))
                .collect(),
        };
        for map in &self.joints {
            let si = source
                .joints
                .iter()
                .position(|j| j.key == map.source)
                .ok_or_else(|| invalid("/retarget/source", "Missing source joint."))?;
            let ti = target
                .joints
                .iter()
                .position(|j| j.key == map.target)
                .ok_or_else(|| invalid("/retarget/target", "Missing target joint."))?;
            if !seen.insert(ti) || !map.translation_scale.is_finite() || map.translation_scale <= 0.
            {
                return Err(invalid(
                    "/retarget/map",
                    "Duplicate target or invalid scale.",
                ));
            }
            let s = &source.joints[si].bind;
            let t = &target.joints[ti].bind;
            let p = &pose.joints[si];
            p.validate()?;
            result.joints[ti] = JointPose {
                translation: (Vec3::from_array(t.translation)
                    + (Vec3::from_array(p.translation) - Vec3::from_array(s.translation))
                        * map.translation_scale)
                    .to_array(),
                rotation: (Quat::from_array(t.rotation)
                    * Quat::from_array(s.rotation).inverse()
                    * Quat::from_array(p.rotation))
                .normalize()
                .to_array(),
                scale: (Vec3::from_array(t.scale) * Vec3::from_array(p.scale)
                    / Vec3::from_array(s.scale))
                .to_array(),
            };
        }
        Ok(result)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationLod3D {
    pub visual_interval: u32,
}
impl AnimationLod3D {
    /// The simulation and gameplay events always advance; only optional visual work skips.
    pub fn evaluate_visual(self, tick: u64, visible: bool) -> bool {
        visible && tick.is_multiple_of(u64::from(self.visual_interval.max(1)))
    }
}
