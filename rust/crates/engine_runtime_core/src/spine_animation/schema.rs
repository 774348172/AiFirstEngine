//! Package-owned controller data. Names are retained for SDK/inspect; evaluation uses indices.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedSpineController {
    pub asset_id: String,
    pub skeleton_asset_id: String,
    pub default_action_index: u16,
    pub parameters: Vec<Parameter>,
    pub layers: Vec<Layer>,
    pub masks: Vec<Mask>,
    pub actions: Vec<Action>,
    pub rules: Vec<Rule>,
    pub sockets: Vec<Socket>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Parameter {
    Bool { name: String, default: bool },
    Trigger { name: String },
}
impl Parameter {
    pub fn name(&self) -> &str {
        match self {
            Self::Bool { name, .. } | Self::Trigger { name } => name,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub name: String,
    pub additive: bool,
    pub mask_index: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mask {
    pub name: String,
    pub bones: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    pub name: String,
    pub source_animation_index: u16,
    pub layer_index: u8,
    pub playback: Playback,
    pub speed_permille: u16,
    pub requested_mix_in_seconds: f64,
    pub mix_in_ticks: u32,
    pub requested_fade_out_seconds: Option<f64>,
    pub fade_out_ticks: Option<u32>,
    pub on_complete: Completion,
    pub mask_index: Option<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Playback {
    Loop,
    Once,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Completion {
    Hold,
    FadeOutLayer,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub all: Vec<(u8, bool)>,
    pub action_index: u16,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Socket {
    pub name: String,
    pub bone_index: u16,
    pub offset: [f32; 3],
}

fn names<'a>(iter: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    iter.into_iter().all(|n| valid_name(n) && seen.insert(n))
}
pub fn ticks(seconds: f64, rate: u32) -> Result<u32, String> {
    if !seconds.is_finite() || !(0.0..=60.0).contains(&seconds) || !(1..=1000).contains(&rate) {
        return Err("animator.binding.invalid_duration".into());
    }
    let product = seconds * rate as f64;
    let nearest = product.round();
    let ulp = if product == 0.0 {
        f64::from_bits(1)
    } else {
        f64::from_bits(product.to_bits() + 1) - product
    };
    let fixed = if (product - nearest).abs() <= 4.0 * ulp {
        nearest
    } else {
        product
    };
    Ok(if seconds == 0.0 {
        0
    } else {
        (fixed.ceil() as u32).max(1)
    })
}
impl CookedSpineController {
    pub fn validate(&self, asset: &super::CookedSpineAsset, rate: u32) -> Result<(), String> {
        let bad = || "animator.binding.invalid_controller".to_string();
        if !valid_name(&self.asset_id)
            || self.skeleton_asset_id != asset.asset_id
            || self.layers.is_empty()
            || self.layers.len() > 4
            || self.parameters.len() > 16
            || self.masks.len() > 16
            || self.actions.is_empty()
            || self.actions.len() > 128
            || self.rules.len() > 64
            || self.sockets.len() > 32
            || !names(self.parameters.iter().map(Parameter::name))
            || !names(self.layers.iter().map(|v| v.name.as_str()))
            || !names(self.masks.iter().map(|v| v.name.as_str()))
            || !names(self.actions.iter().map(|v| v.name.as_str()))
            || !names(self.sockets.iter().map(|v| v.name.as_str()))
            || self.layers[0].additive
            || self
                .actions
                .get(self.default_action_index as usize)
                .is_none_or(|a| a.layer_index != 0)
        {
            return Err(bad());
        }
        for m in &self.masks {
            if m.bones.is_empty()
                || m.bones.len() > 256
                || m.bones.iter().collect::<BTreeSet<_>>().len() != m.bones.len()
                || m.bones.iter().any(|b| !asset.bones.contains(b))
            {
                return Err("animator.mask.dependency_conflict".into());
            }
        }
        for l in &self.layers {
            if l.mask_index.is_some_and(|i| i as usize >= self.masks.len()) {
                return Err(bad());
            }
        }
        for a in &self.actions {
            if a.source_animation_index as usize >= asset.animations.len()
                || a.layer_index as usize >= self.layers.len()
                || a.speed_permille > 4000
                || a.mix_in_ticks != ticks(a.requested_mix_in_seconds, rate)?
                || a.fade_out_ticks
                    != a.requested_fade_out_seconds
                        .map(|s| ticks(s, rate))
                        .transpose()?
                || a.mask_index != self.layers[a.layer_index as usize].mask_index
                || (a.on_complete == Completion::FadeOutLayer
                    && (a.playback != Playback::Once || a.fade_out_ticks.is_none()))
                || (a.on_complete == Completion::Hold && a.fade_out_ticks.is_some())
            {
                return Err(bad());
            }
        }
        for r in &self.rules {
            if r.action_index as usize >= self.actions.len()
                || r.all.is_empty()
                || r.all.len() > 8
                || r.all.iter().map(|v| v.0).collect::<BTreeSet<_>>().len() != r.all.len()
                || r.all.iter().any(|(p, v)| {
                    self.parameters
                        .get(*p as usize)
                        .is_none_or(|p| matches!(p, Parameter::Trigger { .. }) && !v)
                })
            {
                return Err(bad());
            }
        }
        if self.sockets.iter().any(|s| {
            s.bone_index as usize >= asset.bones.len() || s.offset.iter().any(|v| !v.is_finite())
        }) {
            return Err(bad());
        }
        Ok(())
    }
    #[cfg(feature = "spine")]
    pub fn compile(
        &self,
        source: &engine_spine_runtime::import::ValidatedSource,
        rate: u32,
        ppu: f32,
    ) -> Result<std::rc::Rc<super::control::ControlTable>, String> {
        use super::control as c;
        let def = c::Definition {
            tick_rate: rate,
            layers: self
                .layers
                .iter()
                .map(|l| c::Layer {
                    additive: l.additive,
                    weight_permille: 1000,
                })
                .collect(),
            parameters: self
                .parameters
                .iter()
                .map(|p| match p {
                    Parameter::Bool { default, .. } => c::Parameter::Bool(*default),
                    Parameter::Trigger { .. } => c::Parameter::Trigger,
                })
                .collect(),
            masks: self.masks.iter().map(|m| m.bones.clone()).collect(),
            actions: self
                .actions
                .iter()
                .map(|a| c::Action {
                    clip: a.source_animation_index,
                    layer: a.layer_index,
                    looping: a.playback == Playback::Loop,
                    speed_permille: a.speed_permille,
                    mix_in_ticks: a.mix_in_ticks,
                    fade_out_ticks: a.fade_out_ticks,
                    mask: a.mask_index,
                })
                .collect(),
            rules: self
                .rules
                .iter()
                .map(|r| c::Rule {
                    all: r.all.clone(),
                    action: r.action_index,
                })
                .collect(),
            sockets: self
                .sockets
                .iter()
                .map(|s| c::Socket {
                    bone: s.bone_index,
                    offset: s.offset,
                })
                .collect(),
            default_action: self.default_action_index,
            transitions: Vec::new(),
            pixels_per_unit: ppu,
            source_asset_id: self.skeleton_asset_id.clone(),
        };
        c::ControlTable::compile(source, def)
            .map(std::rc::Rc::new)
            .map_err(|e| e.0.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spine374_d_seconds_ceil_preserves_zero_tiny_positive_and_integer_boundary() {
        for (seconds, expected) in [
            (0., 0),
            (1e-30, 1),
            (0.1, 6),
            (0.10000000000000002, 6),
            (0.10000001, 7),
            (60., 3600),
        ] {
            assert_eq!(ticks(seconds, 60).unwrap(), expected);
        }
        for seconds in [-0.1, 60.1, f64::NAN, f64::INFINITY] {
            assert!(ticks(seconds, 60).is_err());
        }
        assert!(ticks(1., 0).is_err());
        assert!(ticks(1., 1001).is_err());
    }
}
