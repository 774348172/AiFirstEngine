//! Spatial audio math and PCM cursor shared by actual Windows/Android sinks.
use crate::audio::DecodedAudioClip;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialAudioSource3D {
    pub schema_version: String,
    pub min_distance: f32,
    pub max_distance: f32,
}
impl SpatialAudioSource3D {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "spatial-audio3d.v1"
            || !self.min_distance.is_finite()
            || !self.max_distance.is_finite()
            || self.min_distance <= 0.
            || self.max_distance <= self.min_distance
            || self.max_distance > 32768.
        {
            return Err("Invalid spatial audio distance range.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialAudioListener3D {
    pub schema_version: String,
    pub enabled: bool,
}
impl SpatialAudioListener3D {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "audio-listener3d.v1" {
            return Err("Unsupported spatial listener.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialAudioGains3D {
    pub left: f32,
    pub right: f32,
}
impl SpatialAudioGains3D {
    pub fn validate(self) -> Result<Self, String> {
        if [self.left, self.right]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.).contains(v))
        {
            return Err("Invalid stereo spatial gain.".into());
        }
        Ok(self)
    }
}
/// Linear distance falloff and constant-power stereo panning in listener space.
/// World positions are f64; subtraction precedes any conversion to f32.
pub fn spatial_audio_gains(
    listener: [f64; 3],
    right: [f32; 3],
    source: [f64; 3],
    config: &SpatialAudioSource3D,
) -> Result<SpatialAudioGains3D, String> {
    config.validate()?;
    if listener.iter().chain(&source).any(|v| !v.is_finite())
        || right.iter().any(|v| !v.is_finite())
    {
        return Err("Non-finite spatial audio pose.".into());
    }
    let direction = std::array::from_fn::<_, 3, _>(|i| source[i] - listener[i]);
    let distance = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    let length = right.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 1e-6 || !distance.is_finite() {
        return Err("Degenerate listener orientation or position.".into());
    }
    let pan = if distance < 1e-6 {
        0.
    } else {
        (0..3)
            .map(|i| direction[i] / distance * f64::from(right[i] / length))
            .sum::<f64>()
            .clamp(-1., 1.)
    };
    let attenuation = ((f64::from(config.max_distance)
        - distance.max(f64::from(config.min_distance)))
        / f64::from(config.max_distance - config.min_distance))
    .clamp(0., 1.);
    Ok(SpatialAudioGains3D {
        left: (((1. - pan) * 0.5).sqrt() * attenuation) as f32,
        right: (((1. + pan) * 0.5).sqrt() * attenuation) as f32,
    })
}
/// A pair is one atomic publication; the audio callback never locks the World.
#[derive(Debug)]
pub struct SpatialGainControl(AtomicU64);
impl Default for SpatialGainControl {
    fn default() -> Self {
        Self(AtomicU64::new(u64::MAX))
    }
}
impl SpatialGainControl {
    pub fn set(&self, gains: Option<SpatialAudioGains3D>) -> Result<(), String> {
        let packed = if let Some(g) = gains {
            g.validate()?;
            u64::from(g.left.to_bits()) | u64::from(g.right.to_bits()) << 32
        } else {
            u64::MAX
        };
        self.0.store(packed, Ordering::Relaxed);
        Ok(())
    }
    fn get(&self) -> Option<SpatialAudioGains3D> {
        let p = self.0.load(Ordering::Relaxed);
        (p != u64::MAX).then(|| SpatialAudioGains3D {
            left: f32::from_bits(p as u32),
            right: f32::from_bits((p >> 32) as u32),
        })
    }
}
/// Stereo output with an independent cursor over shared decoded PCM. Spatial
/// stereo sources are downmixed before panning; nonspatial stereo is preserved.
pub struct SpatialPcmCursor3D {
    pub clip: Arc<DecodedAudioClip>,
    control: Arc<SpatialGainControl>,
    frame: usize,
    side: usize,
    pair: [f32; 2],
    current: [f32; 2],
    first: bool,
}
impl SpatialPcmCursor3D {
    pub fn new(clip: Arc<DecodedAudioClip>, control: Arc<SpatialGainControl>) -> Self {
        Self {
            clip,
            control,
            frame: 0,
            side: 0,
            pair: [0.; 2],
            current: [1.; 2],
            first: true,
        }
    }
    pub fn remaining_samples(&self) -> usize {
        (self.clip.samples.len() / usize::from(self.clip.channels) - self.frame) * 2 - self.side
    }
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(
            self.clip.samples.len() as f64
                / f64::from(self.clip.channels)
                / f64::from(self.clip.sample_rate),
        )
    }
}
impl Iterator for SpatialPcmCursor3D {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.side == 0 {
            let channels = usize::from(self.clip.channels);
            let left = *self.clip.samples.get(self.frame * channels)?;
            let right = if channels == 1 {
                left
            } else {
                self.clip.samples[self.frame * channels + 1]
            };
            if let Some(gain) = self.control.get() {
                let target = [gain.left, gain.right];
                let smoothing = 1. - (-1. / (0.005 * self.clip.sample_rate as f32)).exp();
                for (current, target) in self.current.iter_mut().zip(target) {
                    *current = if self.first {
                        target
                    } else {
                        *current + (target - *current) * smoothing
                    };
                }
                let mono = (left + right) * 0.5;
                self.pair = [mono * self.current[0], mono * self.current[1]];
            } else {
                self.pair = [left, right];
                self.current = [1.; 2];
            }
            self.first = false;
        }
        let sample = self.pair[self.side];
        self.side += 1;
        if self.side == 2 {
            self.side = 0;
            self.frame += 1;
        }
        Some(sample)
    }
}

pub(super) fn project_spatial_audio(
    world: &crate::world::World,
) -> Result<std::collections::BTreeMap<crate::ids::RuntimeEntityId, SpatialAudioGains3D>, String> {
    use crate::{
        archetype::ComponentValue, components::ComponentTypeId, ids::EntityId, query::QuerySpec,
    };
    let data = |id: &EntityId, ty: &str| match world.component_value(id, &ComponentTypeId::new(ty))
    {
        Some(ComponentValue::Dynamic { value, .. }) => {
            Some(crate::character_animation::dynamic_json(&value))
        }
        _ => None,
    };
    let enabled = |id: &EntityId| {
        world.entity(id).is_some_and(|e| e.alive && e.enabled)
            && world.spatial_world(id).is_some_and(|s| s.enabled)
    };
    let position = |id: &EntityId| {
        world
            .spatial_world(id)
            .map(|s| {
                std::array::from_fn::<_, 3, _>(|i| s.anchor[i] + f64::from(s.world_matrix[12 + i]))
            })
            .ok_or_else(|| "Spatial audio requires committed Transform3D.".to_string())
    };
    let mut listener = None;
    for id in world.query_entities(&QuerySpec::all([ComponentTypeId::new(
        "engine.audio_listener3d",
    )])) {
        if !enabled(&id) {
            continue;
        }
        let config: SpatialAudioListener3D = serde_json::from_value(
            data(&id, "engine.audio_listener3d").ok_or("Missing listener data")?,
        )
        .map_err(|e| e.to_string())?;
        config.validate()?;
        if !config.enabled {
            continue;
        }
        if listener.is_some() {
            return Err("Only one enabled spatial audio listener may own output.".into());
        }
        let state = world
            .spatial_world(&id)
            .ok_or("Missing listener spatial pose")?;
        listener = Some((
            position(&id)?,
            [
                state.world_matrix[0],
                state.world_matrix[1],
                state.world_matrix[2],
            ],
        ));
    }
    let source_ids = world.query_entities(&QuerySpec::all([ComponentTypeId::new(
        "engine.spatial_audio3d",
    )]));
    if source_ids.len() > 256 {
        return Err("Spatial audio source budget exceeded.".into());
    }
    let mut output = std::collections::BTreeMap::new();
    for id in source_ids {
        if !enabled(&id) {
            continue;
        }
        if world.audio_source(&id).is_none() {
            return Err("Spatial audio requires an existing AudioSource.".into());
        }
        let config: SpatialAudioSource3D = serde_json::from_value(
            data(&id, "engine.spatial_audio3d").ok_or("Missing source spatial data")?,
        )
        .map_err(|e| e.to_string())?;
        config.validate()?;
        let gains = if let Some((listener, right)) = listener {
            spatial_audio_gains(listener, right, position(&id)?, &config)?
        } else {
            SpatialAudioGains3D {
                left: 0.,
                right: 0.,
            }
        };
        output.insert(
            world
                .runtime_id_for_source(&id)
                .ok_or("Stale spatial audio source")?,
            gains,
        );
    }
    Ok(output)
}
