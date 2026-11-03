//! Heightfield authoring contracts and compact vegetation instance data.
//!
//! Heightfields are lowered offline to the existing mesh/collision/navigation
//! assets. They do not create another runtime terrain or resource owner.
use crate::navigation_3d::AgentProfile3D;
use crate::spatial_contract::{ContractResult, SpatialContractError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(test, path = "terrain3d/mesh.rs")]
mod mesh;
pub use mesh::TerrainCollisionMesh3D;
#[cfg_attr(test, path = "terrain3d/vegetation.rs")]
mod vegetation;
pub use vegetation::*;

pub const HEIGHTFIELD_RECIPE_3D: &str = "heightfield.edge-locked-fan.v1";
pub const MAX_TERRAIN_TILES: usize = 64;
pub const MAX_TERRAIN_QUADS: u16 = 256;
pub const MAX_TERRAIN_MATERIAL_LAYERS: usize = 4;

pub(super) fn invalid(path: &str, message: impl Into<String>) -> SpatialContractError {
    SpatialContractError::new("terrain3d_invalid", path, message)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainAssetIdentity3D {
    pub asset_id: String,
    pub asset_guid: String,
}
impl TerrainAssetIdentity3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.asset_id.trim().is_empty()
            || self.asset_guid.trim().is_empty()
            || self.asset_id.len() > 256
            || self.asset_guid.len() > 256
        {
            return Err(invalid(
                "/identity",
                "Expected bounded explicit asset id and GUID.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainLodOutput3D {
    /// Power-of-two grid stride. All outer boundary samples remain present.
    pub step: u16,
    pub mesh: TerrainAssetIdentity3D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainNavigationOutput3D {
    pub asset: TerrainAssetIdentity3D,
    pub profile: AgentProfile3D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeightfieldTile3D {
    pub key: String,
    /// Offline neighbour identity, independent of the runtime cell origin.
    pub coordinate: [i32; 2],
    /// Row-major (quads+1)^2 samples in metres, local Y.
    pub heights: Vec<f32>,
    /// Row-major quads^2 material-layer indices. Empty selects layer zero.
    #[serde(default)]
    pub cell_layers: Vec<u8>,
    pub lods: Vec<TerrainLodOutput3D>,
    #[serde(default)]
    pub collision: Option<TerrainAssetIdentity3D>,
    #[serde(default)]
    pub navigation: Option<TerrainNavigationOutput3D>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeightfieldAsset3D {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub quads: u16,
    pub sample_spacing: f32,
    /// One to four existing material slots; selection is per cell, not an
    /// accepted-but-ignored blend map. Renderers bind ordinary materials.
    pub material_slots: Vec<String>,
    pub tiles: Vec<HeightfieldTile3D>,
}

impl HeightfieldAsset3D {
    pub fn tile_width(&self) -> f32 {
        f32::from(self.quads) * self.sample_spacing
    }

    pub fn validate(&self) -> ContractResult<()> {
        TerrainAssetIdentity3D {
            asset_id: self.asset_id.clone(),
            asset_guid: self.asset_guid.clone(),
        }
        .validate()?;
        if self.schema_version != "terrain3d-asset.v1"
            || self.quads < 2
            || self.quads > MAX_TERRAIN_QUADS
            || !self.quads.is_power_of_two()
            || !self.sample_spacing.is_finite()
            || !(0.01..=8.).contains(&self.sample_spacing)
            || self.tile_width() > 2048.
            || self.tiles.is_empty()
            || self.tiles.len() > MAX_TERRAIN_TILES
            || self.material_slots.is_empty()
            || self.material_slots.len() > MAX_TERRAIN_MATERIAL_LAYERS
        {
            return Err(invalid(
                "/grid",
                "Invalid schema, grid, layer or tile budget.",
            ));
        }
        let mut slots = BTreeSet::new();
        if self
            .material_slots
            .iter()
            .any(|slot| slot.trim().is_empty() || slot.len() > 128 || !slots.insert(slot))
        {
            return Err(invalid(
                "/materialSlots",
                "Material slots must be unique bounded names.",
            ));
        }
        let n = usize::from(self.quads);
        let mut keys = BTreeSet::new();
        let mut coordinates = BTreeSet::new();
        let mut ids = BTreeSet::from([self.asset_id.as_str()]);
        let mut guids = BTreeSet::from([self.asset_guid.as_str()]);
        for (index, tile) in self.tiles.iter().enumerate() {
            let path = format!("/tiles/{index}");
            if tile.key.trim().is_empty()
                || tile.key.len() > 128
                || !keys.insert(&tile.key)
                || !coordinates.insert(tile.coordinate)
                || tile.coordinate.iter().any(|v| v.unsigned_abs() > 1_000_000)
                || tile.heights.len() != (n + 1) * (n + 1)
                || tile
                    .heights
                    .iter()
                    .any(|h| !h.is_finite() || h.abs() > 2048.)
                || (!tile.cell_layers.is_empty() && tile.cell_layers.len() != n * n)
                || tile
                    .cell_layers
                    .iter()
                    .any(|v| usize::from(*v) >= self.material_slots.len())
                || tile.lods.is_empty()
                || tile.lods.len() > 9
            {
                return Err(invalid(
                    &path,
                    "Invalid tile identity, heights, material cells or output budget.",
                ));
            }
            let mut previous = 0;
            for lod in &tile.lods {
                if lod.step == 0
                    || !lod.step.is_power_of_two()
                    || lod.step > self.quads
                    || lod.step <= previous
                {
                    return Err(invalid(
                        &path,
                        "LOD steps must be increasing powers of two within the grid.",
                    ));
                }
                previous = lod.step;
            }
            if tile.lods[0].step != 1 {
                return Err(invalid(&path, "The full-density LOD is required."));
            }
            if let Some(nav) = &tile.navigation {
                nav.profile.validate()?;
                if (nav.profile.tile_width - self.tile_width()).abs() > 1e-5 {
                    return Err(invalid(
                        &path,
                        "Navigation tile width must match the terrain tile.",
                    ));
                }
            }
            let identities = tile
                .lods
                .iter()
                .map(|lod| &lod.mesh)
                .chain(tile.collision.iter())
                .chain(tile.navigation.iter().map(|nav| &nav.asset));
            for identity in identities {
                identity.validate()?;
                if !ids.insert(&identity.asset_id) || !guids.insert(&identity.asset_guid) {
                    return Err(invalid(
                        &path,
                        "Derived output asset IDs and GUIDs must be unique.",
                    ));
                }
            }
        }
        self.sample_lattice()?;
        Ok(())
    }

    /// A short-lived offline projection of source samples. Duplicate samples
    /// are shared boundaries and must agree exactly before any output is made.
    pub(super) fn sample_lattice(&self) -> ContractResult<BTreeMap<[i64; 2], f32>> {
        let n = i64::from(self.quads);
        let side = usize::from(self.quads) + 1;
        let mut samples = BTreeMap::new();
        for tile in &self.tiles {
            for z in 0..=self.quads {
                for x in 0..=self.quads {
                    let key = [
                        i64::from(tile.coordinate[0]) * n + i64::from(x),
                        i64::from(tile.coordinate[1]) * n + i64::from(z),
                    ];
                    let Some(height) = tile.heights.get(usize::from(z) * side + usize::from(x))
                    else {
                        return Err(invalid(
                            "/heights",
                            "Height sample count does not match grid.",
                        ));
                    };
                    if samples
                        .insert(key, *height)
                        .is_some_and(|old| old != *height)
                    {
                        return Err(invalid(
                            "/tiles/edge",
                            "Adjacent tile edge heights disagree.",
                        ));
                    }
                }
            }
        }
        Ok(samples)
    }

    pub fn tile(&self, key: &str) -> ContractResult<&HeightfieldTile3D> {
        self.tiles
            .iter()
            .find(|tile| tile.key == key)
            .ok_or_else(|| invalid("/tile", "Unknown terrain tile key."))
    }
}
