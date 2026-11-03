//! Render-only representations. Selection never changes gameplay entity identity
//! or the character animation update rate.
use crate::spatial_contract::{ContractResult, MeshRenderer3D, SpatialContractError};
use serde::{Deserialize, Serialize};
fn invalid(message: &str) -> SpatialContractError {
    SpatialContractError::new("model_lod3d.invalid", "/representation", message)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelLodLevel3D {
    pub distance: f64,
    pub renderer: MeshRenderer3D,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelLod3D {
    pub schema_version: String,
    pub levels: Vec<ModelLodLevel3D>,
}
impl ModelLod3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "model-lod3d.v1"
            || self.levels.is_empty()
            || self.levels.len() > 8
            || self.levels[0].distance != 0.
        {
            return Err(invalid(
                "Expected 1..8 ordered LODs, starting at distance zero.",
            ));
        }
        for (i, l) in self.levels.iter().enumerate() {
            if !l.distance.is_finite()
                || !(0.0..=1_000_000.).contains(&l.distance)
                || i > 0 && l.distance <= self.levels[i - 1].distance
            {
                return Err(invalid(
                    "LOD distances must be finite and strictly increase.",
                ));
            }
            l.renderer.validate()?;
        }
        Ok(())
    }
    pub fn select(&self, distance: f64) -> usize {
        self.levels
            .partition_point(|l| l.distance <= distance)
            .saturating_sub(1)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HlodRole3D {
    Detail,
    Proxy,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hlod3D {
    pub schema_version: String,
    pub group: String,
    pub center: [f64; 3],
    pub distance: f64,
    pub role: HlodRole3D,
}
impl Hlod3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.schema_version != "hlod3d.v1"
            || self.group.trim().is_empty()
            || self.group.len() > 128
            || self.center.iter().any(|n| !n.is_finite())
            || !self.distance.is_finite()
            || !(0.1..=1_000_000.).contains(&self.distance)
        {
            return Err(invalid(
                "HLOD needs a bounded group, finite center and switch distance.",
            ));
        }
        Ok(())
    }
    pub fn visible(&self, camera: [f64; 3]) -> bool {
        let distance = (0..3)
            .map(|i| (camera[i] - self.center[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        match self.role {
            HlodRole3D::Detail => distance < self.distance,
            HlodRole3D::Proxy => distance >= self.distance,
        }
    }
}
/// Runtime snapshot metadata shared by submeshes; assets remain with Prepare.
#[derive(Debug, Clone, Default, PartialEq)]
#[doc(hidden)]
pub struct MeshPresentation3D {
    pub lod: Option<ModelLod3D>,
    pub hlod: Option<Hlod3D>,
    pub vegetation: Option<crate::terrain3d::VegetationBinding3D>,
    pub time_seconds: f64,
    pub range: Option<(f64, Option<f64>)>,
    pub source_instance: Option<String>,
}
impl MeshPresentation3D {
    pub(crate) fn visible(&self, camera: [f64; 3], position: [f64; 3]) -> bool {
        if self.hlod.as_ref().is_some_and(|h| !h.visible(camera)) {
            return false;
        }
        let distance = (0..3)
            .map(|i| (position[i] - camera[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        self.range
            .is_none_or(|(min, max)| distance >= min && max.is_none_or(|max| distance < max))
    }
}

/// Validate the same data channel at authoring, hydration and deferred mutation.
pub(crate) fn validate_component(
    name: &str,
    value: serde_json::Value,
) -> Option<ContractResult<()>> {
    fn decode<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> ContractResult<T> {
        serde_json::from_value(v).map_err(|e| invalid(&e.to_string()))
    }
    Some(match name {
        "engine.model_lod3d" => decode::<ModelLod3D>(value).and_then(|v| v.validate()),
        "engine.hlod3d" => decode::<Hlod3D>(value).and_then(|v| v.validate()),
        "engine.vegetation3d" => {
            decode::<crate::terrain3d::VegetationBinding3D>(value).and_then(|v| {
                v.cluster_asset.validate()?;
                let mut ids = std::collections::BTreeSet::new();
                if v.promoted_instance_ids.len() > crate::terrain3d::MAX_VEGETATION_INSTANCES
                    || v.promoted_instance_ids
                        .iter()
                        .any(|id| id.is_empty() || id.len() > 256 || !ids.insert(id))
                {
                    return Err(invalid(
                        "Vegetation promotion identities must be unique and bounded.",
                    ));
                }
                Ok(())
            })
        }
        _ => return None,
    })
}

/// Authoring and runtime use this public entry without duplicating schemas.
pub fn validate_presentation_component(name: &str, value: serde_json::Value) -> ContractResult<()> {
    validate_component(name, value)
        .unwrap_or_else(|| Err(invalid("Unknown presentation component.")))
}
