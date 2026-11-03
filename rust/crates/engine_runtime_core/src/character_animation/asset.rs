use super::*;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CHARACTER_RECIPE: &str = "character3d.gltf.v1";
pub const MAX_JOINTS: usize = 1024;
pub const MAX_KEYS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}
impl Default for JointPose {
    fn default() -> Self {
        Self {
            translation: [0.; 3],
            rotation: [0., 0., 0., 1.],
            scale: [1.; 3],
        }
    }
}
impl JointPose {
    pub fn matrix(&self) -> [f32; 16] {
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_array(self.rotation),
            Vec3::from_array(self.translation),
        )
        .to_cols_array()
    }
    pub fn validate(&self) -> ContractResult<()> {
        if self
            .translation
            .iter()
            .chain(&self.scale)
            .chain(&self.rotation)
            .any(|v| !v.is_finite())
            || self.scale.contains(&0.)
            || (Quat::from_array(self.rotation).length() - 1.).abs() > 1e-4
        {
            return Err(invalid(
                "/pose",
                "Expected finite invertible TRS and unit quaternion.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterJoint {
    /// Committed import-node key, never a display name or transient node index.
    pub key: String,
    pub parent: Option<u16>,
    pub bind: JointPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackProperty {
    Translation,
    Rotation,
    Scale,
    MorphWeights,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyInterpolation {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTrack {
    pub joint: u16,
    pub property: TrackProperty,
    pub interpolation: KeyInterpolation,
    pub times: Vec<f32>,
    pub width: u16,
    /// Tightly packed glTF order; cubic keys are [in tangent, value, out tangent].
    pub values: Vec<f32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationEvent3D {
    pub time: f32,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationClip3D {
    pub key: String,
    pub duration: f32,
    pub tracks: Vec<AnimationTrack>,
    #[serde(default)]
    pub events: Vec<AnimationEvent3D>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VertexInfluence {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphTarget3D {
    pub position_deltas: Vec<[f32; 3]>,
    pub normal_deltas: Vec<[f32; 3]>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterSkin {
    pub mesh_joint: u16,
    pub mesh_asset_id: String,
    pub joints: Vec<u16>,
    pub inverse_bind: Vec<[f32; 16]>,
    pub influences: Vec<VertexInfluence>,
    #[serde(default)]
    pub morph_targets: Vec<MorphTarget3D>,
    #[serde(default)]
    pub morph_weights: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub source_digest: String,
    pub recipe: String,
    pub joints: Vec<CharacterJoint>,
    pub skins: Vec<CharacterSkin>,
    pub clips: Vec<AnimationClip3D>,
}
impl CharacterAsset {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "character3d.v1"
            || self.recipe != CHARACTER_RECIPE
            || self.asset_id.is_empty()
            || !crate::render_contract::is_sha256_digest(&self.source_digest)
            || self.joints.is_empty()
            || self.joints.len() > MAX_JOINTS
            || self.clips.len() > 1024
            || self.skins.len() > 1024
        {
            return Err(invalid(
                "",
                "Invalid character identity, version or count budget.",
            ));
        }
        let mut keys = BTreeSet::new();
        for (i, joint) in self.joints.iter().enumerate() {
            if joint.key.is_empty()
                || !keys.insert(&joint.key)
                || joint.parent.is_some_and(|p| usize::from(p) >= i)
            {
                return Err(invalid(
                    &format!("/joints/{i}"),
                    "Stable joint keys must be unique; parents must precede children.",
                ));
            }
            joint
                .bind
                .validate()
                .map_err(|e| e.at(&format!("/joints/{i}")))?;
        }
        keys.clear();
        let mut total_keys = 0usize;
        for (i, clip) in self.clips.iter().enumerate() {
            let path = format!("/clips/{i}");
            if clip.key.is_empty()
                || !keys.insert(&clip.key)
                || !clip.duration.is_finite()
                || clip.duration <= 0.
                || clip.events.len() > 4096
            {
                return Err(invalid(
                    &path,
                    "Invalid clip key, duration or event budget.",
                ));
            }
            let mut targets = BTreeSet::new();
            for (n, track) in clip.tracks.iter().enumerate() {
                total_keys = total_keys.saturating_add(track.times.len());
                let width = match track.property {
                    TrackProperty::Rotation => 4,
                    TrackProperty::MorphWeights => track.width,
                    _ => 3,
                };
                let samples = if track.interpolation == KeyInterpolation::CubicSpline {
                    3
                } else {
                    1
                };
                if total_keys > MAX_KEYS
                    || usize::from(track.joint) >= self.joints.len()
                    || !targets.insert((track.joint, track.property as u8))
                    || width == 0
                    || width > 32
                    || track.width != width
                    || track.times.is_empty()
                    || track
                        .times
                        .iter()
                        .any(|t| !t.is_finite() || *t < 0. || *t > clip.duration)
                    || track.times.windows(2).any(|p| p[0] >= p[1])
                    || track.times.len().checked_mul(usize::from(width) * samples)
                        != Some(track.values.len())
                    || track.values.iter().any(|v| !v.is_finite())
                {
                    return Err(invalid(
                        &format!("{path}/tracks/{n}"),
                        "Invalid target, key times, dimensions or key budget.",
                    ));
                }
                if track.property == TrackProperty::Rotation {
                    for value in track.values.chunks_exact(4 * samples) {
                        let offset = if samples == 3 { 4 } else { 0 };
                        if (Quat::from_slice(&value[offset..offset + 4]).length() - 1.).abs() > 1e-4
                        {
                            return Err(invalid(&path, "Rotation keys must be unit quaternions."));
                        }
                    }
                }
                if track.property == TrackProperty::MorphWeights
                    && !self.skins.iter().any(|s| {
                        s.mesh_joint == track.joint
                            && s.morph_targets.len() == usize::from(track.width)
                    })
                {
                    return Err(invalid(
                        &path,
                        "Morph track must target a mesh with matching target count.",
                    ));
                }
            }
            if clip.events.iter().any(|e| {
                e.name.is_empty() || !e.time.is_finite() || e.time < 0. || e.time >= clip.duration
            }) || clip.events.windows(2).any(|p| p[0].time > p[1].time)
            {
                return Err(invalid(&format!("{path}/events"),"Events must be ordered in [0,duration); equal-time events retain authored order."));
            }
        }
        for (i, skin) in self.skins.iter().enumerate() {
            let path = format!("/skins/{i}");
            if usize::from(skin.mesh_joint) >= self.joints.len()
                || skin.mesh_asset_id.is_empty()
                || skin.joints.is_empty()
                || skin.joints.len() > MAX_JOINTS
                || skin.joints.len() != skin.inverse_bind.len()
                || skin
                    .joints
                    .iter()
                    .any(|j| usize::from(*j) >= self.joints.len())
                || skin.joints.iter().collect::<BTreeSet<_>>().len() != skin.joints.len()
            {
                return Err(invalid(
                    &path,
                    "Missing/duplicate joints or inverse bind matrices.",
                ));
            }
            for m in &skin.inverse_bind {
                let m = Mat4::from_cols_array(m);
                if !m.is_finite()
                    || m.determinant().abs() < 1e-10
                    || m.row(3).to_array() != [0., 0., 0., 1.]
                {
                    return Err(invalid(
                        &path,
                        "Inverse bind must be finite, affine and invertible.",
                    ));
                }
            }
            for v in &skin.influences {
                if v.joints
                    .iter()
                    .any(|j| usize::from(*j) >= skin.joints.len())
                    || v.weights
                        .iter()
                        .any(|w| !w.is_finite() || *w < 0. || *w > 1.)
                    || (v.weights.iter().sum::<f32>() - 1.).abs() > 1e-4
                {
                    return Err(invalid(&path,"Skin requires four valid joint indices and normalized nonnegative weights."));
                }
            }
            if skin.morph_targets.len() > 32
                || skin.morph_weights.len() != skin.morph_targets.len()
                || skin
                    .morph_weights
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err(invalid(
                    &path,
                    "Morph profile supports at most 32 targets with weights in [0,1].",
                ));
            }
            for target in &skin.morph_targets {
                if target.position_deltas.len() != skin.influences.len()
                    || target.normal_deltas.len() != skin.influences.len()
                    || target
                        .position_deltas
                        .iter()
                        .chain(&target.normal_deltas)
                        .flatten()
                        .any(|v| !v.is_finite())
                {
                    return Err(invalid(
                        &path,
                        "Morph streams must match vertex count and contain finite deltas.",
                    ));
                }
            }
        }
        Ok(())
    }
}
