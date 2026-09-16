use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleEffectError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ParticleEffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}
impl std::error::Error for ParticleEffectError {}

fn require(ok: bool, field: &str, message: &str) -> Result<(), ParticleEffectError> {
    if ok {
        Ok(())
    } else {
        Err(ParticleEffectError {
            field: field.into(),
            message: message.into(),
        })
    }
}
fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}
fn nonnegative(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite() && *v >= 0.0)
}
fn positive(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite() && *v > 0.0)
}
fn color(values: &[f32; 4]) -> bool {
    nonnegative(values) && values[3] <= 1.0
}

pub(super) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !value.starts_with("engine_")
}
fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        && value != "."
        && value != ".."
}
fn named(name: &str, names: &mut BTreeSet<String>, field: &str) -> Result<(), ParticleEffectError> {
    require(
        identifier(name) && names.insert(name.into()),
        field,
        "expected a unique WGSL field name (ASCII letter first; engine_ reserved)",
    )
}
fn value_valid(value: &ParticleValue) -> bool {
    match value {
        ParticleValue::Float(v) => v.is_finite(),
        ParticleValue::Uint(_) => true,
        ParticleValue::Vec3(v) => finite(v),
        ParticleValue::Vec4(v) => finite(v),
    }
}

impl ParticleEffectDescription {
    pub fn validate(&self) -> Result<(), ParticleEffectError> {
        require(
            self.schema_version == PARTICLE_EFFECT_SCHEMA,
            "schemaVersion",
            "expected particle-effect.v1",
        )?;
        require(
            identity(&self.asset_id),
            "assetId",
            "expected a stable asset ID without path separators",
        )?;
        require(
            identity(&self.asset_guid),
            "assetGuid",
            "expected a stable asset GUID",
        )?;
        require(
            finite(&self.bounds.center) && positive(&self.bounds.half_extents),
            "bounds",
            "center must be finite and halfExtents positive meters",
        )?;
        require(
            self.budget.max_particles > 0 && self.budget.max_particles <= 1_000_000,
            "budget.maxParticles",
            "expected 1..1000000",
        )?;
        require(
            self.budget.max_events_per_step <= 65_536,
            "budget.maxEventsPerStep",
            "expected 0..65536",
        )?;
        require(
            positive(&[self.budget.importance]) && self.budget.importance <= 1.0,
            "budget.importance",
            "expected (0,1]",
        )?;
        require(
            positive(&[self.budget.quality_scale]) && self.budget.quality_scale <= 1.0,
            "budget.qualityScale",
            "expected (0,1]",
        )?;
        require(
            !self.emitters.is_empty() && self.emitters.len() <= 64,
            "emitters",
            "expected 1..64 emitters",
        )?;
        require(
            self.parameters.len() <= 64,
            "parameters",
            "at most 64 parameters",
        )?;
        let mut params = BTreeSet::new();
        for (i, param) in self.parameters.iter().enumerate() {
            let field = format!("parameters[{i}]");
            named(&param.name, &mut params, &field)?;
            require(
                value_valid(&param.default),
                &field,
                "default must be finite",
            )?;
            if param.unit == ParticleUnit::LinearColor {
                require(
                    matches!(param.default, ParticleValue::Vec4(v) if color(&v)),
                    &field,
                    "linearColor requires nonnegative vec4 and alpha in [0,1]",
                )?;
            }
            if let Some([min, max]) = param.range {
                require(
                    finite(&[min, max]) && min <= max,
                    &field,
                    "range must be finite and ordered",
                )?;
                let inside = |v: f32| v >= min && v <= max;
                let valid = match param.default {
                    ParticleValue::Float(v) => inside(v),
                    ParticleValue::Uint(v) => {
                        min >= 0.0
                            && min.fract() == 0.0
                            && max.fract() == 0.0
                            && (v as f64) >= min as f64
                            && (v as f64) <= max as f64
                    }
                    ParticleValue::Vec3(v) => v.into_iter().all(inside),
                    ParticleValue::Vec4(v) => v.into_iter().all(inside),
                };
                require(
                    valid,
                    &field,
                    "default is outside the inclusive range or integer range is invalid",
                )?;
            }
        }
        let mut names = BTreeSet::new();
        let mut capacity = 0u64;
        for (i, emitter) in self.emitters.iter().enumerate() {
            let field = format!("emitters[{i}]({})", emitter.name);
            named(&emitter.name, &mut names, &field)?;
            emitter.validate(&field)?;
            capacity += u64::from(emitter.capacity);
        }
        require(
            capacity <= u64::from(self.budget.max_particles),
            "budget.maxParticles",
            "sum of emitter capacities exceeds effect budget",
        )?;
        for (i, emitter) in self.emitters.iter().enumerate() {
            for child in &emitter.child_emission {
                let field = format!("emitters[{i}].childEmission");
                require(
                    names.contains(&child.target_emitter),
                    &field,
                    "targetEmitter does not exist",
                )?;
                require(child.count > 0 && child.count <= self.budget.max_events_per_step && (1..=8).contains(&child.max_generation), &field, "child emission requires a positive count within event budget and maxGeneration 1..8")?;
            }
        }
        Ok(())
    }

    pub fn asset_refs(&self) -> Vec<(&RuntimeAssetRef, &'static str, String)> {
        let mut refs = Vec::new();
        for (index, emitter) in self.emitters.iter().enumerate() {
            if let Some(asset) = &emitter.draw.texture {
                refs.push((asset, "texture", format!("emitters[{index}].draw.texture")));
            }
            if let Some(asset) = &emitter.draw.material {
                refs.push((
                    asset,
                    "material",
                    format!("emitters[{index}].draw.material"),
                ));
            }
            if let ParticleGeometry::Mesh { asset } = &emitter.draw.geometry {
                refs.push((
                    asset,
                    "mesh",
                    format!("emitters[{index}].draw.geometry.asset"),
                ));
            }
        }
        refs
    }
}

impl ParticleEmitter {
    fn validate(&self, field: &str) -> Result<(), ParticleEffectError> {
        require(
            self.capacity > 0 && self.capacity <= 1_000_000,
            &format!("{field}.capacity"),
            "expected 1..1000000",
        )?;
        require(
            positive(&[self.duration_seconds])
                && nonnegative(&[self.delay_seconds, self.rate_per_second]),
            &format!("{field}.timing"),
            "duration must be positive; delay/rate finite and nonnegative",
        )?;
        require(
            positive(&self.lifetime_seconds)
                && self.lifetime_seconds[0] <= self.lifetime_seconds[1],
            &format!("{field}.lifetimeSeconds"),
            "expected ordered positive lifetime range in seconds",
        )?;
        require(
            finite(&self.velocity_min)
                && finite(&self.velocity_max)
                && self
                    .velocity_min
                    .iter()
                    .zip(self.velocity_max)
                    .all(|(a, b)| *a <= b),
            &format!("{field}.velocity"),
            "velocity bounds must be finite and ordered meters per second",
        )?;
        require(
            color(&self.color),
            &format!("{field}.color"),
            "expected nonnegative linear RGB and alpha in [0,1]",
        )?;
        require(
            positive(&self.size_meters) && self.rotation_radians.is_finite(),
            &format!("{field}.appearance"),
            "size must be positive meters and rotation finite radians",
        )?;
        require(
            self.bursts.len() <= 256,
            &format!("{field}.bursts"),
            "at most 256 bursts",
        )?;
        let mut previous = -1.0;
        for burst in &self.bursts {
            require(
                nonnegative(&[burst.time_seconds])
                    && burst.time_seconds < self.duration_seconds
                    && burst.time_seconds >= previous
                    && burst.count > 0,
                &format!("{field}.bursts"),
                "bursts must be ordered, have positive counts, and lie in [0,durationSeconds)",
            )?;
            previous = burst.time_seconds;
        }
        let shape_valid = match self.shape {
            ParticleShape::Point {} => true,
            ParticleShape::Sphere { radius } => positive(&[radius]),
            ParticleShape::Box { half_extents } => positive(&half_extents),
            ParticleShape::Cone {
                radius,
                angle_radians,
            } => {
                positive(&[radius])
                    && nonnegative(&[angle_radians])
                    && angle_radians <= std::f32::consts::FRAC_PI_2
            }
        };
        require(
            shape_valid,
            &format!("{field}.shape"),
            "invalid shape dimensions or angle",
        )?;
        let integrations = self
            .updates
            .iter()
            .filter(|v| matches!(v, ParticleUpdate::Integrate {}))
            .count();
        require(
            (self.custom_owns_position && integrations == 0 && self.behavior_source.is_some())
                || (!self.custom_owns_position && integrations == 1),
            &format!("{field}.updates"),
            "include one integrate, or customOwnsPosition with behaviorSource and no integrate",
        )?;
        require(
            self.updates.len() <= 32,
            &format!("{field}.updates"),
            "at most 32 ordered updates",
        )?;
        for (i, update) in self.updates.iter().enumerate() {
            let at = format!("{field}.updates[{i}]");
            match update {
                ParticleUpdate::Integrate {} => (),
                ParticleUpdate::Gravity { acceleration } => {
                    require(finite(acceleration), &at, "acceleration must be finite")?
                }
                ParticleUpdate::Drag { coefficient } => require(
                    nonnegative(&[*coefficient]),
                    &at,
                    "drag must be finite and nonnegative",
                )?,
                ParticleUpdate::Noise {
                    amplitude,
                    frequency,
                } => require(
                    nonnegative(&[*amplitude, *frequency]),
                    &at,
                    "noise must be finite and nonnegative",
                )?,
                ParticleUpdate::Rotate { radians_per_second } => require(
                    radians_per_second.is_finite(),
                    &at,
                    "angular speed must be finite",
                )?,
                ParticleUpdate::SizeOverLife { keys } => {
                    curve(keys.iter().map(|k| k.time), &at)?;
                    require(
                        keys.iter().all(|k| nonnegative(&[k.value])),
                        &at,
                        "size multipliers must be finite and nonnegative",
                    )?;
                }
                ParticleUpdate::ColorOverLife { keys } => {
                    curve(keys.iter().map(|k| k.time), &at)?;
                    require(
                        keys.iter().all(|k| color(&k.value)),
                        &at,
                        "invalid linear color key",
                    )?;
                }
            }
        }
        require(
            self.custom_state.len() <= 16 && self.inputs.len() <= 16,
            &format!("{field}.behavior"),
            "at most 16 state fields and 16 inputs",
        )?;
        let mut names = BTreeSet::new();
        for value in &self.custom_state {
            named(&value.name, &mut names, &format!("{field}.customState"))?;
            require(
                value_valid(&value.default),
                field,
                "state default must be finite",
            )?;
        }
        names.clear();
        for value in &self.inputs {
            named(&value.name, &mut names, &format!("{field}.inputs"))?;
            require(value_valid(&value.value), field, "input must be finite")?;
        }
        if let Some(flip) = &self.draw.flipbook {
            require(
                self.draw.texture.is_some()
                    && flip.columns > 0
                    && flip.rows > 0
                    && u64::from(flip.columns) * u64::from(flip.rows) <= 4096
                    && positive(&[flip.fps]),
                &format!("{field}.draw.flipbook"),
                "flipbook requires a texture, 1..4096 cells and positive fps",
            )?;
        }
        if let Some(trail) = &self.draw.trail {
            require(
                (2..=64).contains(&trail.segments)
                    && positive(&[trail.lifetime_seconds, trail.width_meters]),
                &format!("{field}.draw.trail"),
                "trail requires 2..64 segments, positive lifetime/width",
            )?;
        }
        require(
            self.collisions.len() <= 16 && self.child_emission.len() <= 16,
            &format!("{field}.events"),
            "at most 16 colliders and 16 child emission rules",
        )?;
        for collision in &self.collisions {
            require(
                nonnegative(&[collision.restitution]) && collision.restitution <= 1.0,
                &format!("{field}.collisions"),
                "restitution must be [0,1]",
            )?;
            let valid = match collision.shape {
                ParticleCollider::Plane { normal, distance } => {
                    finite(&normal)
                        && distance.is_finite()
                        && (normal.into_iter().map(|x| x * x).sum::<f32>() - 1.0).abs() <= 1e-4
                }
                ParticleCollider::Sphere { center, radius } => {
                    finite(&center) && positive(&[radius])
                }
            };
            require(
                valid,
                &format!("{field}.collisions"),
                "plane needs a unit normal; sphere needs finite center and positive radius",
            )?;
        }
        Ok(())
    }
}

fn curve(times: impl Iterator<Item = f32>, field: &str) -> Result<(), ParticleEffectError> {
    let times: Vec<_> = times.collect();
    require(
        (2..=32).contains(&times.len())
            && times.first() == Some(&0.0)
            && times.last() == Some(&1.0)
            && finite(&times)
            && times.windows(2).all(|pair| pair[0] < pair[1]),
        field,
        "curve requires 2..32 strictly increasing normalized keys from 0 to 1",
    )
}
