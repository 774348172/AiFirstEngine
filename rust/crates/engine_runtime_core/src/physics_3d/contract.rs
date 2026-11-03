//! Cooked, backend-neutral Physics3D contracts. All units are SI.
use crate::spatial_contract::{ContractResult, SpatialContractError};
use serde::{Deserialize, Serialize};
pub(super) fn invalid(path: &str, message: impl Into<String>) -> SpatialContractError {
    SpatialContractError::new("physics3d_invalid", path, message)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CollisionShape3D {
    Asset {
        source: crate::spatial_contract::SpatialAssetRef,
    },
    Box {
        half_extents: [f32; 3],
    },
    Sphere {
        radius: f32,
    },
    Capsule {
        half_height: f32,
        radius: f32,
    },
    Convex {
        vertices: Vec<[f32; 3]>,
    },
    StaticMesh {
        vertices: Vec<[f32; 3]>,
        triangles: Vec<[u32; 3]>,
    },
}
impl CollisionShape3D {
    pub fn validate(&self) -> ContractResult<()> {
        let positive = |v: f32| v.is_finite() && v > 0. && v <= 4096.;
        let vertices_ok = |v: &Vec<[f32; 3]>| {
            v.len() <= 1_000_000
                && v.iter()
                    .flatten()
                    .all(|x| x.is_finite() && x.abs() <= 4096.)
        };
        let valid = match self {
            Self::Asset { source } => {
                source.0.asset_type == "collision3d"
                    && !source.0.id.trim().is_empty()
                    && source.0.guid.as_ref().is_none_or(|g| !g.trim().is_empty())
            }
            Self::Box { half_extents } => half_extents.iter().all(|v| positive(*v)),
            Self::Sphere { radius } => positive(*radius),
            Self::Capsule {
                half_height,
                radius,
            } => {
                positive(*radius)
                    && half_height.is_finite()
                    && *half_height >= 0.
                    && *half_height <= 4096.
            }
            Self::Convex { vertices } => {
                vertices.len() >= 4 && vertices.len() <= 4096 && vertices_ok(vertices)
            }
            Self::StaticMesh {
                vertices,
                triangles,
            } => {
                vertices.len() >= 3
                    && vertices_ok(vertices)
                    && !triangles.is_empty()
                    && triangles.len() <= 1_000_000
                    && triangles.iter().all(|t| {
                        t.iter().all(|i| (*i as usize) < vertices.len())
                            && t[0] != t[1]
                            && t[1] != t[2]
                            && t[0] != t[2]
                    })
            }
        };
        if !valid {
            return Err(invalid(
                "/shape",
                "Invalid or over-budget collision geometry.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedCollision3D {
    pub schema_version: String,
    pub shape: CollisionShape3D,
}
impl CookedCollision3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "collision3d.v1" {
            return Err(invalid("/schemaVersion", "Unsupported collision asset."));
        }
        if matches!(self.shape, CollisionShape3D::Asset { .. }) {
            return Err(invalid(
                "/shape",
                "Cooked collision cannot reference another collision asset.",
            ));
        }
        self.shape.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BodyMotion3D {
    Static,
    Dynamic,
    Kinematic,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Body3D {
    #[serde(default = "yes")]
    pub enabled: bool,
    pub schema_version: String,
    pub motion: BodyMotion3D,
    pub shape: CollisionShape3D,
    #[serde(default = "default_mass")]
    pub mass: f32,
    #[serde(default = "default_friction")]
    pub friction: f32,
    #[serde(default)]
    pub restitution: f32,
    #[serde(default)]
    pub ccd: bool,
    #[serde(default)]
    pub sensor: bool,
    #[serde(default = "yes")]
    pub allow_sleeping: bool,
}
fn default_mass() -> f32 {
    1.
}
fn default_friction() -> f32 {
    0.5
}
fn yes() -> bool {
    true
}
impl Body3D {
    pub fn validate(&self) -> ContractResult<()> {
        self.shape.validate()?;
        if self.schema_version != "body3d.v1"
            || (self.sensor && self.ccd)
            || !self.mass.is_finite()
            || self.mass <= 0.
            || self.mass > 1e6
            || !self.friction.is_finite()
            || !(0.0..=10.0).contains(&self.friction)
            || !self.restitution.is_finite()
            || !(0.0..=1.0).contains(&self.restitution)
            || (matches!(self.shape, CollisionShape3D::StaticMesh { .. })
                && (self.motion != BodyMotion3D::Static || self.sensor))
        {
            return Err(invalid(
                "/body",
                "Invalid body material/mass or non-static triangle mesh.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterMotor3D {
    #[serde(default = "yes")]
    pub enabled: bool,
    pub schema_version: String,
    pub radius: f32,
    pub half_height: f32,
    pub max_slope_degrees: f32,
    pub step_height: f32,
    #[serde(default = "default_mass")]
    pub mass: f32,
    #[serde(default = "default_gravity")]
    pub gravity: f32,
}
fn default_gravity() -> f32 {
    9.81
}
impl CharacterMotor3D {
    pub fn validate(&self) -> ContractResult<()> {
        CollisionShape3D::Capsule {
            half_height: self.half_height,
            radius: self.radius,
        }
        .validate()?;
        if self.schema_version != "motor3d.v1"
            || !self.max_slope_degrees.is_finite()
            || !(1.0..89.0).contains(&self.max_slope_degrees)
            || !self.step_height.is_finite()
            || !(0.0..=self.radius * 2.).contains(&self.step_height)
            || !self.mass.is_finite()
            || self.mass <= 0.
            || !self.gravity.is_finite()
            || !(0.0..=100.).contains(&self.gravity)
        {
            return Err(invalid(
                "/motor",
                "Invalid motor dimensions/slope/step/mass/gravity.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsRay3D {
    pub origin: [f64; 3],
    pub displacement: [f32; 3],
}
impl PhysicsRay3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.origin.iter().any(|v| !v.is_finite())
            || self.displacement.iter().any(|v| !v.is_finite())
            || self.displacement.iter().map(|v| v * v).sum::<f32>() <= 1e-12
        {
            return Err(invalid("/query", "Invalid bounded ray."));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsHit3D {
    pub entity: String,
    pub generation: u64,
    pub fraction: f32,
    pub distance: f32,
    pub position: [f64; 3],
    pub normal: [f32; 3],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicsOverlap3D {
    pub entity: String,
    pub generation: u64,
    pub penetration: f32,
    pub position: [f64; 3],
    pub normal: [f32; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JointKind3D {
    Fixed,
    Point,
}
/// The pivot is relative to the owning body's origin, in authored metres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BodyTarget3D {
    pub entity_ref: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Joint3D {
    pub schema_version: String,
    pub kind: JointKind3D,
    pub other: BodyTarget3D,
    pub pivot: [f32; 3],
}
impl Joint3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "joint3d.v1"
            || self.other.entity_ref.trim().is_empty()
            || self.pivot.iter().any(|v| !v.is_finite() || v.abs() > 100.)
        {
            return Err(invalid("/joint", "Invalid fixed/point joint or pivot."));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotorResult3D {
    pub generation: u64,
    pub tick: u64,
    pub requested: [f32; 3],
    pub accepted: [f32; 3],
    pub grounded: bool,
    pub velocity: [f32; 3],
}
