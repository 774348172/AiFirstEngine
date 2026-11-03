use crate::spatial_contract::{ContractResult, SpatialAssetRef, SpatialContractError};
use serde::{Deserialize, Serialize};
pub(super) fn invalid(path: &str, message: impl Into<String>) -> SpatialContractError {
    SpatialContractError::new("navigation3d_invalid", path, message)
}
pub const NAV_RECIPE: &str = "recast1.6.0-detour64-monotone.v1";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentProfile3D {
    pub key: String,
    pub radius: f32,
    pub height: f32,
    pub climb: f32,
    pub max_slope_degrees: f32,
    pub cell_size: f32,
    pub cell_height: f32,
    pub tile_width: f32,
}
impl AgentProfile3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.key.trim().is_empty()
            || self.key.len() > 128
            || [
                self.radius,
                self.height,
                self.climb,
                self.max_slope_degrees,
                self.cell_size,
                self.cell_height,
                self.tile_width,
            ]
            .iter()
            .any(|v| !v.is_finite())
            || !(0.05..=4.).contains(&self.radius)
            || !(0.2..=8.).contains(&self.height)
            || !(0.0..=2.).contains(&self.climb)
            || !(1.0..=85.).contains(&self.max_slope_degrees)
            || !(0.025..=1.).contains(&self.cell_size)
            || !(0.01..=0.5).contains(&self.cell_height)
            || !(4.0..=64.).contains(&self.tile_width)
            || (self.tile_width / self.cell_size).round() > 960.
            || ((self.tile_width / self.cell_size).round() * self.cell_size - self.tile_width).abs()
                > 0.001
            || (self.height / self.cell_height).ceil() < 3.
        {
            return Err(invalid(
                "/profile",
                "Invalid or over-budget agent bake profile.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedNavTile3D {
    pub x: i32,
    pub z: i32,
    pub data: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedNavigation3D {
    pub schema_version: String,
    pub asset_id: String,
    pub source_digest: String,
    pub recipe: String,
    pub profile: AgentProfile3D,
    pub tiles: Vec<CookedNavTile3D>,
}
impl CookedNavigation3D {
    pub fn validate(&self) -> ContractResult<()> {
        self.profile.validate()?;
        let mut seen = std::collections::BTreeSet::new();
        if self.schema_version != "navigation3d.v1"
            || self.asset_id.trim().is_empty()
            || self.recipe != NAV_RECIPE
            || self.source_digest.len() != 71
            || !self.source_digest.starts_with("sha256:")
            || self.tiles.is_empty()
            || self.tiles.len() > 64
            || self.tiles.iter().any(|t| {
                t.x.unsigned_abs() > 2048
                    || t.z.unsigned_abs() > 2048
                    || t.data.is_empty()
                    || t.data.len() > 16 * 1024 * 1024
                    || !seen.insert((t.x, t.z))
            })
            || self.tiles.iter().map(|t| t.data.len()).sum::<usize>() > 64 * 1024 * 1024
        {
            return Err(invalid(
                "/tiles",
                "Invalid cooked navigation identity/tiles or byte budget.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationSurface3D {
    pub schema_version: String,
    pub navigation: SpatialAssetRef,
}
impl NavigationSurface3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "navigation3d-binding.v1"
            || self.navigation.0.asset_type != "navigation3d"
            || self.navigation.0.id.trim().is_empty()
        {
            return Err(invalid(
                "/surface",
                "Expected a cooked navigation AssetRef.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationAgent3D {
    pub schema_version: String,
    pub profile: String,
    pub speed: f32,
    pub arrival_distance: f32,
    pub avoidance_radius: f32,
    pub max_neighbours: u32,
}
impl NavigationAgent3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "navigation3d-agent.v1"
            || self.profile.trim().is_empty()
            || !self.speed.is_finite()
            || !(0.01..=30.).contains(&self.speed)
            || !self.arrival_distance.is_finite()
            || !(0.01..=2.).contains(&self.arrival_distance)
            || !self.avoidance_radius.is_finite()
            || !(0.0..=8.).contains(&self.avoidance_radius)
            || self.max_neighbours > 32
        {
            return Err(invalid(
                "/agent",
                "Invalid navigation speed/arrival/avoidance budget.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationObstacle3D {
    pub schema_version: String,
    pub half_extents: [f32; 3],
}
impl NavigationObstacle3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "navigation3d-obstacle.v1"
            || self
                .half_extents
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 128.)
        {
            return Err(invalid(
                "/obstacle",
                "Invalid loaded-region obstacle bounds.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NavigationStatus3D {
    Pending,
    Following,
    LinkRequired,
    Arrived,
    Cancelled,
    TimedOut,
    Invalidated,
    Unreachable,
    NotResident,
    Capacity,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationState3D {
    pub revision: u64,
    pub status: NavigationStatus3D,
    pub points: Vec<[f64; 3]>,
    pub off_mesh: Vec<bool>,
    pub accepted_tick: u64,
}
