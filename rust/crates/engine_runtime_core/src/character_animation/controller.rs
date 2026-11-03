use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationState3D {
    pub key: String,
    pub clip: String,
    pub looping: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTransition3D {
    pub from: String,
    pub to: String,
    pub parameter: String,
    pub minimum: f32,
    pub blend_seconds: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationGraph3D {
    pub initial: String,
    pub parameters: BTreeMap<String, f32>,
    pub states: Vec<AnimationState3D>,
    pub transitions: Vec<AnimationTransition3D>,
    #[serde(default)]
    pub layers: Vec<AnimationLayer3D>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationLayer3D {
    pub clip: String,
    pub weight: f32,
    #[serde(default)]
    pub weight_parameter: Option<String>,
    #[serde(default)]
    pub mask: Vec<String>,
    #[serde(default)]
    pub additive: bool,
}
impl AnimationGraph3D {
    pub fn validate(&self, asset: &CharacterAsset) -> ContractResult<()> {
        if self.states.is_empty()
            || self.states.len() > 256
            || self.transitions.len() > 1024
            || self.parameters.len() > 128
            || self
                .parameters
                .iter()
                .any(|(k, v)| k.is_empty() || !v.is_finite())
        {
            return Err(invalid("/graph", "Graph budget or parameter invalid."));
        }
        let mut keys = BTreeSet::new();
        for state in &self.states {
            if state.key.is_empty()
                || !keys.insert(&state.key)
                || !asset.clips.iter().any(|c| c.key == state.clip)
            {
                return Err(invalid("/states", "Missing clip or duplicate state key."));
            }
        }
        if !keys.contains(&self.initial) {
            return Err(invalid("/initial", "Initial state missing."));
        }
        for t in &self.transitions {
            if !keys.contains(&t.from)
                || !keys.contains(&t.to)
                || t.from == t.to
                || !self.parameters.contains_key(&t.parameter)
                || !t.minimum.is_finite()
                || !t.blend_seconds.is_finite()
                || t.blend_seconds < 0.
            {
                return Err(invalid(
                    "/transitions",
                    "Invalid transition target, parameter or duration.",
                ));
            }
        }
        if self.layers.len() > 16
            || self.layers.iter().any(|l| {
                !asset.clips.iter().any(|c| c.key == l.clip)
                    || !l.weight.is_finite()
                    || !(0.0..=1.0).contains(&l.weight)
                    || l.weight_parameter
                        .as_ref()
                        .is_some_and(|p| !self.parameters.contains_key(p))
                    || l.mask
                        .iter()
                        .any(|key| !asset.joints.iter().any(|j| &j.key == key))
            })
        {
            return Err(invalid(
                "/layers",
                "Invalid layer clip, weight, parameter or joint mask.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Animator3D {
    asset: Arc<CharacterAsset>,
    graph: Arc<AnimationGraph3D>,
    parameters: BTreeMap<String, f32>,
    state: usize,
    clock: ClipClock,
    outgoing: Option<(usize, ClipClock, f32, f32)>,
    last_tick: Option<u64>,
    pub pose: SampledCharacterPose,
    pub events: Vec<AnimationEventOccurrence>,
    root_joint: Option<usize>,
    pub root_displacement: [f32; 3],
}
impl Animator3D {
    pub fn new(asset: Arc<CharacterAsset>, graph: AnimationGraph3D) -> ContractResult<Self> {
        asset.validate()?;
        graph.validate(&asset)?;
        let state = graph
            .states
            .iter()
            .position(|s| s.key == graph.initial)
            .unwrap();
        let clip = asset
            .clips
            .iter()
            .position(|c| c.key == graph.states[state].clip)
            .unwrap();
        let pose = asset.sample(clip, 0.)?;
        let looping = graph.states[state].looping;
        let parameters = graph.parameters.clone();
        Ok(Self {
            asset,
            graph: Arc::new(graph),
            parameters,
            state,
            clock: ClipClock::new(looping),
            outgoing: None,
            last_tick: None,
            pose,
            events: Vec::new(),
            root_joint: None,
            root_displacement: [0.; 3],
        })
    }
    pub fn set_parameter(&mut self, name: &str, value: f32) -> ContractResult<()> {
        if !value.is_finite() {
            return Err(invalid("/parameter", "Parameter must be finite."));
        }
        *self
            .parameters
            .get_mut(name)
            .ok_or_else(|| invalid("/parameter", "Undeclared parameter."))? = value;
        Ok(())
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.clock.paused = paused;
        if let Some((_, clock, _, _)) = &mut self.outgoing {
            clock.paused = paused;
        }
    }
    pub fn seek(&mut self, time: f64) -> ContractResult<()> {
        self.clock.seek(time)?;
        self.outgoing = None;
        self.events.clear();
        let clip = self.clip_index(self.state);
        self.pose = self
            .asset
            .sample(clip, self.clock.sample_time(&self.asset.clips[clip]))?;
        Ok(())
    }
    pub fn state(&self) -> &str {
        &self.graph.states[self.state].key
    }
    pub fn play(&mut self, state: &str, reset: bool) -> ContractResult<()> {
        let next = self
            .graph
            .states
            .iter()
            .position(|s| s.key == state)
            .ok_or_else(|| invalid("/state", "Undeclared animation state."))?;
        if reset || next != self.state {
            self.state = next;
            self.clock = ClipClock::new(self.graph.states[next].looping);
            self.outgoing = None;
            self.events.clear();
        }
        self.set_paused(false);
        Ok(())
    }
    pub fn asset(&self) -> &Arc<CharacterAsset> {
        &self.asset
    }
    pub fn track_root_motion(&mut self, joint: Option<usize>) -> ContractResult<()> {
        if joint
            .is_some_and(|i| i >= self.asset.joints.len() || self.asset.joints[i].parent.is_some())
        {
            return Err(invalid(
                "/rootMotion/joint",
                "Root motion target must be a skeleton root.",
            ));
        }
        self.root_joint = joint;
        Ok(())
    }
    fn clip_index(&self, state: usize) -> usize {
        self.asset
            .clips
            .iter()
            .position(|c| c.key == self.graph.states[state].clip)
            .unwrap()
    }
    /// One committed simulation tick; presentation rates do not drive this clock.
    pub fn tick(&mut self, tick: u64, dt: f32) -> ContractResult<()> {
        if self.last_tick.is_some_and(|last| tick <= last) || !dt.is_finite() || dt < 0. {
            return Err(invalid(
                "/tick",
                "Ticks must increase and delta must be finite/nonnegative.",
            ));
        }
        // Work on a candidate so failed sampling/event budgets never partially publish.
        let mut candidate = self.clone();
        candidate.advance(tick, dt)?;
        *self = candidate;
        Ok(())
    }
    fn advance(&mut self, tick: u64, dt: f32) -> ContractResult<()> {
        if !self.clock.paused {
            if let Some(t) = self
                .graph
                .transitions
                .iter()
                .find(|t| t.from == self.state() && self.parameters[&t.parameter] >= t.minimum)
                .cloned()
            {
                let to = self
                    .graph
                    .states
                    .iter()
                    .position(|s| s.key == t.to)
                    .unwrap();
                self.outgoing = (t.blend_seconds > 0.)
                    .then(|| (self.state, self.clock.clone(), 0., t.blend_seconds));
                self.state = to;
                self.clock = ClipClock::new(self.graph.states[to].looping);
            }
        }
        let clip = self.clip_index(self.state);
        let old_time = self.clock.time;
        self.events = self.clock.advance(&self.asset.clips[clip], f64::from(dt))?;
        self.root_displacement = self
            .root_joint
            .map(|joint| {
                super::root_motion::translation_interval(
                    &self.asset,
                    clip,
                    joint,
                    old_time,
                    self.clock.time,
                    self.clock.looping,
                )
            })
            .transpose()?
            .unwrap_or([0.; 3]);
        self.pose = self
            .asset
            .sample(clip, self.clock.sample_time(&self.asset.clips[clip]))?;
        if let Some((state, mut clock, mut elapsed, duration)) = self.outgoing.take() {
            let clip = self.clip_index(state);
            // Only the destination state emits gameplay events during a transition.
            let old_time = clock.time;
            clock.advance(&self.asset.clips[clip], f64::from(dt))?;
            if !clock.paused {
                elapsed += dt;
            }
            let mut old = self
                .asset
                .sample(clip, clock.sample_time(&self.asset.clips[clip]))?;
            blend_pose(
                &mut old,
                &self.pose,
                (elapsed / duration).min(1.),
                &[],
                None,
            )?;
            if let Some(joint) = self.root_joint {
                let previous = super::root_motion::translation_interval(
                    &self.asset,
                    clip,
                    joint,
                    old_time,
                    clock.time,
                    clock.looping,
                )?;
                let weight = (elapsed / duration).min(1.);
                for (value, previous) in self.root_displacement.iter_mut().zip(previous) {
                    *value = previous + (*value - previous) * weight;
                }
            }
            self.pose = old;
            if elapsed < duration {
                self.outgoing = Some((state, clock, elapsed, duration));
            }
        }
        for layer in &self.graph.layers {
            let index = self
                .asset
                .clips
                .iter()
                .position(|c| c.key == layer.clip)
                .unwrap();
            let clip = &self.asset.clips[index];
            let pose = self.asset.sample(
                index,
                self.clock.time.rem_euclid(f64::from(clip.duration)) as f32,
            )?;
            let reference = layer
                .additive
                .then(|| self.asset.sample(index, 0.))
                .transpose()?;
            let mask = layer
                .mask
                .iter()
                .map(|key| {
                    self.asset
                        .joints
                        .iter()
                        .position(|j| &j.key == key)
                        .unwrap() as u16
                })
                .collect::<Vec<_>>();
            let weight = layer
                .weight_parameter
                .as_ref()
                .map_or(layer.weight, |p| self.parameters[p]);
            blend_pose(&mut self.pose, &pose, weight, &mask, reference.as_ref())?;
            if let Some(joint) = self
                .root_joint
                .filter(|j| mask.is_empty() || mask.contains(&(*j as u16)))
            {
                let delta = super::root_motion::translation_interval(
                    &self.asset,
                    index,
                    joint,
                    old_time,
                    self.clock.time,
                    true,
                )?;
                for (value, delta) in self.root_displacement.iter_mut().zip(delta) {
                    if layer.additive {
                        *value += delta * weight;
                    } else {
                        *value += (delta - *value) * weight;
                    }
                }
            }
        }
        self.last_tick = Some(tick);
        Ok(())
    }
}
