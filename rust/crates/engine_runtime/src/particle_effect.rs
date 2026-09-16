//! Particle authoring and cooked data. GPU resources remain renderer-owned.
use crate::runtime_package::RuntimeAssetRef;
use serde::{Deserialize, Serialize};

pub mod program;
mod validation;
pub use validation::ParticleEffectError;

pub const PARTICLE_EFFECT_SCHEMA: &str = "particle-effect.v1";
pub const PARTICLE_PROGRAM_CONTRACT: &str = "particle-program.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleEffectDescription {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    #[serde(default)]
    pub seed: u32,
    pub bounds: ParticleBounds,
    pub budget: ParticleBudget,
    #[serde(default)]
    pub parameters: Vec<ParticleParameter>,
    pub emitters: Vec<ParticleEmitter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleBounds {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleBudget {
    pub max_particles: u32,
    pub max_events_per_step: u32,
    #[serde(default = "one")]
    pub importance: f32,
    #[serde(default = "one")]
    pub quality_scale: f32,
    pub offscreen: ParticleOffscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleOffscreen {
    Simulate,
    Pause,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleSpace {
    Local,
    World,
}

/// Tagged values are the only parameter type declaration; WGSL fields are generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleValue {
    Float(f32),
    Uint(u32),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl ParticleValue {
    pub fn wgsl_type(&self) -> &'static str {
        match self {
            Self::Float(_) => "f32",
            Self::Uint(_) => "u32",
            Self::Vec3(_) => "vec3<f32>",
            Self::Vec4(_) => "vec4<f32>",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleParameterStage {
    Spawn,
    Update,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleUnit {
    Unitless,
    Seconds,
    Meters,
    Radians,
    MetersPerSecond,
    LinearColor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleParameter {
    pub name: String,
    pub default: ParticleValue,
    pub unit: ParticleUnit,
    pub stage: ParticleParameterStage,
    /// Inclusive component-wise range; integer parameters use an integral range.
    #[serde(default)]
    pub range: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleStateField {
    pub name: String,
    pub default: ParticleValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleEmitter {
    pub name: String,
    pub capacity: u32,
    pub space: ParticleSpace,
    pub duration_seconds: f32,
    #[serde(default)]
    pub delay_seconds: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub rate_per_second: f32,
    #[serde(default)]
    pub bursts: Vec<ParticleBurst>,
    pub lifetime_seconds: [f32; 2],
    pub shape: ParticleShape,
    pub velocity_min: [f32; 3],
    pub velocity_max: [f32; 3],
    pub color: [f32; 4],
    pub size_meters: [f32; 2],
    #[serde(default)]
    pub rotation_radians: f32,
    /// Ordered standard updates. Include Integrate exactly once unless custom owns position.
    pub updates: Vec<ParticleUpdate>,
    #[serde(default)]
    pub custom_owns_position: bool,
    #[serde(default)]
    pub behavior_source: Option<String>,
    #[serde(default)]
    pub custom_state: Vec<ParticleStateField>,
    #[serde(default)]
    pub inputs: Vec<ParticleInput>,
    pub draw: ParticleDraw,
    #[serde(default)]
    pub collisions: Vec<ParticleCollision>,
    #[serde(default)]
    pub child_emission: Vec<ParticleChildEmission>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleBurst {
    pub time_seconds: f32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleShape {
    Point {},
    Sphere { radius: f32 },
    Box { half_extents: [f32; 3] },
    Cone { radius: f32, angle_radians: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleUpdate {
    Integrate {},
    Gravity { acceleration: [f32; 3] },
    Drag { coefficient: f32 },
    Noise { amplitude: f32, frequency: f32 },
    ColorOverLife { keys: Vec<ParticleColorKey> },
    SizeOverLife { keys: Vec<ParticleScalarKey> },
    Rotate { radians_per_second: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleScalarKey {
    pub time: f32,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleColorKey {
    pub time: f32,
    pub value: [f32; 4],
}

/// Named read-only explicit environment values; no access to ECS or GPU bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleInput {
    pub name: String,
    pub value: ParticleValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleBlend {
    Alpha,
    Additive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleSort {
    None,
    BackToFront,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleDraw {
    pub geometry: ParticleGeometry,
    pub blend: ParticleBlend,
    pub sort: ParticleSort,
    #[serde(default)]
    pub texture: Option<RuntimeAssetRef>,
    #[serde(default)]
    pub material: Option<RuntimeAssetRef>,
    #[serde(default)]
    pub flipbook: Option<ParticleFlipbook>,
    #[serde(default)]
    pub trail: Option<ParticleTrail>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleGeometry {
    Billboard2d {},
    Billboard3d {},
    Mesh { asset: RuntimeAssetRef },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleFlipbook {
    pub columns: u32,
    pub rows: u32,
    pub fps: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleTrail {
    pub segments: u32,
    pub lifetime_seconds: f32,
    pub width_meters: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleCollider {
    Plane { normal: [f32; 3], distance: f32 },
    Sphere { center: [f32; 3], radius: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleCollision {
    pub shape: ParticleCollider,
    pub restitution: f32,
    pub kill: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleEvent {
    Spawn,
    Death,
    Collision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleChildEmission {
    pub event: ParticleEvent,
    pub target_emitter: String,
    pub count: u32,
    pub max_generation: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedParticleProgram {
    pub emitter: String,
    pub wgsl: String,
    pub author_path: Option<String>,
    pub author_first_line: u32,
    pub author_line_count: u32,
    pub particle_stride: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedParticleEffect {
    pub program_contract: String,
    pub description: ParticleEffectDescription,
    pub programs: Vec<CookedParticleProgram>,
    /// Content digests of author files and referenced assets, keyed by source path.
    pub source_digests: std::collections::BTreeMap<String, String>,
}

fn one() -> f32 {
    1.0
}

#[cfg(test)]
mod tests;
