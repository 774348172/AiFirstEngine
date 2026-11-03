use super::*;
use glam::{Mat4, Quat};

#[derive(Debug, Clone, PartialEq)]
pub struct SampledCharacterPose {
    pub joints: Vec<JointPose>,
    pub morph_weights: Vec<(u16, Vec<f32>)>,
}

impl AnimationTrack {
    pub fn sample(&self, time: f32) -> ContractResult<Vec<f32>> {
        if !time.is_finite() || self.times.is_empty() {
            return Err(invalid(
                "/time",
                "Cannot sample an empty track or nonfinite time.",
            ));
        }
        let width = usize::from(self.width);
        let cubic = self.interpolation == KeyInterpolation::CubicSpline;
        let stride = width * if cubic { 3 } else { 1 };
        if width == 0
            || self.times.len().checked_mul(stride) != Some(self.values.len())
            || self
                .times
                .windows(2)
                .any(|p| !p[0].is_finite() || p[0] >= p[1])
            || self.values.iter().any(|v| !v.is_finite())
        {
            return Err(invalid("/track", "Invalid track layout or key data."));
        }
        let at = |i: usize, part: usize| {
            &self.values[i * stride + part * width..i * stride + (part + 1) * width]
        };
        let value = usize::from(cubic);
        let right = self.times.partition_point(|t| *t <= time);
        if right == 0 {
            return Ok(at(0, value).to_vec());
        }
        if right == self.times.len() {
            return Ok(at(right - 1, value).to_vec());
        }
        let left = right - 1;
        let dt = self.times[right] - self.times[left];
        let t = (time - self.times[left]) / dt;
        let a = at(left, value);
        let b = at(right, value);
        if self.interpolation == KeyInterpolation::Step {
            return Ok(a.to_vec());
        }
        if !cubic && self.property == TrackProperty::Rotation {
            return Ok(Quat::from_slice(a)
                .slerp(Quat::from_slice(b), t)
                .normalize()
                .to_array()
                .to_vec());
        }
        let mut out = if cubic {
            let out_tangent = at(left, 2);
            let in_tangent = at(right, 0);
            let t2 = t * t;
            let t3 = t2 * t;
            (0..width)
                .map(|i| {
                    (2. * t3 - 3. * t2 + 1.) * a[i]
                        + (t3 - 2. * t2 + t) * dt * out_tangent[i]
                        + (-2. * t3 + 3. * t2) * b[i]
                        + (t3 - t2) * dt * in_tangent[i]
                })
                .collect::<Vec<_>>()
        } else {
            (0..width).map(|i| a[i] + (b[i] - a[i]) * t).collect()
        };
        if self.property == TrackProperty::Rotation {
            let q = Quat::from_slice(&out);
            if !q.is_finite() || q.length_squared() < 1e-12 {
                return Err(invalid(
                    "/rotation",
                    "Cubic rotation interpolates to a degenerate quaternion.",
                ));
            }
            out = q.normalize().to_array().to_vec();
        }
        if out.iter().any(|v| !v.is_finite()) {
            return Err(invalid("/sample", "Interpolated key is not finite."));
        }
        Ok(out)
    }
}

impl CharacterAsset {
    /// Asset validation is performed once when loading, not once per sampled joint.
    pub fn sample(&self, clip: usize, time: f32) -> ContractResult<SampledCharacterPose> {
        let clip = self
            .clips
            .get(clip)
            .ok_or_else(|| invalid("/clip", "Clip does not exist."))?;
        let mut pose = SampledCharacterPose {
            joints: self.joints.iter().map(|j| j.bind.clone()).collect(),
            morph_weights: self
                .skins
                .iter()
                .map(|s| (s.mesh_joint, s.morph_weights.clone()))
                .collect(),
        };
        for track in &clip.tracks {
            let value = track.sample(time)?;
            let joint = pose
                .joints
                .get_mut(usize::from(track.joint))
                .ok_or_else(|| invalid("/joint", "Missing target joint."))?;
            match track.property {
                TrackProperty::Translation => {
                    joint.translation = value
                        .try_into()
                        .map_err(|_| invalid("/translation", "Expected width 3."))?
                }
                TrackProperty::Rotation => {
                    joint.rotation = value
                        .try_into()
                        .map_err(|_| invalid("/rotation", "Expected width 4."))?
                }
                TrackProperty::Scale => {
                    joint.scale = value
                        .try_into()
                        .map_err(|_| invalid("/scale", "Expected width 3."))?
                }
                TrackProperty::MorphWeights => {
                    if value.iter().any(|v| !(0.0..=1.0).contains(v)) {
                        return Err(invalid("/morphWeights", "Sampled weight outside [0,1]."));
                    }
                    for (target, weights) in &mut pose.morph_weights {
                        if *target == track.joint {
                            *weights = value.clone();
                        }
                    }
                }
            }
        }
        for joint in &pose.joints {
            joint.validate()?;
        }
        Ok(pose)
    }
    pub fn global_matrices(&self, pose: &SampledCharacterPose) -> ContractResult<Vec<[f32; 16]>> {
        if pose.joints.len() != self.joints.len() {
            return Err(invalid("/pose", "Skeleton/pose count mismatch."));
        }
        let mut globals = Vec::with_capacity(pose.joints.len());
        for (index, joint) in pose.joints.iter().enumerate() {
            let local = Mat4::from_cols_array(&joint.matrix());
            let matrix = if let Some(parent) = self.joints[index].parent {
                Mat4::from_cols_array(
                    globals
                        .get(usize::from(parent))
                        .ok_or_else(|| invalid("/parent", "Non-topological skeleton."))?,
                ) * local
            } else {
                local
            };
            if !matrix.is_finite() {
                return Err(invalid("/pose", "Global transform overflow."));
            }
            globals.push(matrix.to_cols_array());
        }
        Ok(globals)
    }
    pub fn skin_matrices(
        &self,
        skin: usize,
        pose: &SampledCharacterPose,
    ) -> ContractResult<Vec<[f32; 16]>> {
        let skin = self
            .skins
            .get(skin)
            .ok_or_else(|| invalid("/skin", "Missing skin."))?;
        let globals = self.global_matrices(pose)?;
        let mesh = Mat4::from_cols_array(&globals[usize::from(skin.mesh_joint)]);
        if mesh.determinant().abs() < 1e-10 {
            return Err(invalid("/skin", "Mesh transform is singular."));
        }
        let inverse = mesh.inverse();
        Ok(skin
            .joints
            .iter()
            .zip(&skin.inverse_bind)
            .map(|(j, bind)| {
                (inverse
                    * Mat4::from_cols_array(&globals[usize::from(*j)])
                    * Mat4::from_cols_array(bind))
                .to_cols_array()
            })
            .collect())
    }
}

/// Reference to a specific traversal, including repeated loop occurrences.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEventOccurrence {
    pub name: String,
    pub cycle: u64,
    pub time: f32,
}
#[derive(Debug, Clone)]
pub struct ClipClock {
    pub time: f64,
    pub looping: bool,
    pub paused: bool,
    started: bool,
}
impl ClipClock {
    pub fn new(looping: bool) -> Self {
        Self {
            time: 0.,
            looping,
            paused: false,
            started: false,
        }
    }
    /// Seek changes pose only. It neither replays skipped events nor fires the destination.
    pub fn seek(&mut self, time: f64) -> ContractResult<()> {
        if !time.is_finite() || time < 0. {
            return Err(invalid("/seek", "Seek requires finite nonnegative time."));
        }
        self.time = time;
        self.started = true;
        Ok(())
    }
    pub fn sample_time(&self, clip: &AnimationClip3D) -> f32 {
        if self.looping {
            self.time.rem_euclid(f64::from(clip.duration)) as f32
        } else {
            self.time.min(f64::from(clip.duration)) as f32
        }
    }
    pub fn advance(
        &mut self,
        clip: &AnimationClip3D,
        dt: f64,
    ) -> ContractResult<Vec<AnimationEventOccurrence>> {
        if !dt.is_finite() || dt < 0. || !clip.duration.is_finite() || clip.duration <= 0. {
            return Err(invalid(
                "/delta",
                "Clock requires finite positive duration and nonnegative delta.",
            ));
        }
        if self.paused || dt == 0. {
            return Ok(Vec::new());
        }
        let duration = f64::from(clip.duration);
        let next = if self.looping {
            self.time + dt
        } else {
            (self.time + dt).min(duration)
        };
        if !next.is_finite()
            || next / duration > u64::MAX as f64
            || next - self.time > duration * 4096.
        {
            return Err(invalid("/delta", "Traversal exceeds bounded loop budget."));
        }
        let first = (self.time / duration).floor() as u64;
        let last = if self.looping {
            (next / duration).floor() as u64
        } else {
            0
        };
        let mut events = Vec::new();
        for cycle in first..=last {
            for event in &clip.events {
                let absolute = cycle as f64 * duration + f64::from(event.time);
                if (absolute > self.time || (!self.started && absolute == 0.)) && absolute <= next {
                    if events.len() == 4096 {
                        return Err(invalid(
                            "/events",
                            "Traversal exceeds 4096-event budget; clock unchanged.",
                        ));
                    }
                    events.push(AnimationEventOccurrence {
                        name: event.name.clone(),
                        cycle,
                        time: event.time,
                    });
                }
            }
        }
        self.time = next;
        self.started = true;
        Ok(events)
    }
}

pub fn blend_pose(
    base: &mut SampledCharacterPose,
    layer: &SampledCharacterPose,
    weight: f32,
    mask: &[u16],
    additive_reference: Option<&SampledCharacterPose>,
) -> ContractResult<()> {
    if !weight.is_finite()
        || !(0.0..=1.0).contains(&weight)
        || base.joints.len() != layer.joints.len()
        || mask.iter().any(|j| usize::from(*j) >= base.joints.len())
        || additive_reference.is_some_and(|r| r.joints.len() != base.joints.len())
    {
        return Err(invalid("/blend", "Invalid weight, mask or skeleton."));
    }
    for (i, (a, b)) in base.joints.iter_mut().zip(&layer.joints).enumerate() {
        if !mask.is_empty() && !mask.contains(&(i as u16)) {
            continue;
        }
        if let Some(reference) = additive_reference {
            let r = &reference.joints[i];
            for c in 0..3 {
                a.translation[c] += (b.translation[c] - r.translation[c]) * weight;
                a.scale[c] *= 1. + (b.scale[c] / r.scale[c] - 1.) * weight;
            }
            a.rotation = (Quat::from_array(a.rotation)
                * Quat::IDENTITY.slerp(
                    Quat::from_array(r.rotation).inverse() * Quat::from_array(b.rotation),
                    weight,
                ))
            .normalize()
            .to_array();
        } else {
            for c in 0..3 {
                a.translation[c] += (b.translation[c] - a.translation[c]) * weight;
                a.scale[c] += (b.scale[c] - a.scale[c]) * weight;
            }
            a.rotation = Quat::from_array(a.rotation)
                .slerp(Quat::from_array(b.rotation), weight)
                .normalize()
                .to_array();
        }
        a.validate()?;
    }
    for (joint, weights) in &mut base.morph_weights {
        if !mask.is_empty() && !mask.contains(joint) {
            continue;
        }
        if let Some((_, other)) = layer.morph_weights.iter().find(|(j, _)| j == joint) {
            if weights.len() != other.len() {
                return Err(invalid("/morphWeights", "Morph layout mismatch."));
            }
            for (i, (a, b)) in weights.iter_mut().zip(other).enumerate() {
                if let Some(reference) = additive_reference {
                    let r = reference
                        .morph_weights
                        .iter()
                        .find(|(j, _)| j == joint)
                        .and_then(|(_, w)| w.get(i))
                        .ok_or_else(|| invalid("/morphWeights", "Missing additive reference."))?;
                    *a += (*b - *r) * weight;
                } else {
                    *a += (*b - *a) * weight;
                }
                if !(0.0..=1.0).contains(a) {
                    return Err(invalid(
                        "/morphWeights",
                        "Blended morph weight outside [0,1].",
                    ));
                }
            }
        }
    }
    Ok(())
}
