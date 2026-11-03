//! Project light semantics; shadow opt-in is consumed by production depth projections.
use super::*;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Light3D {
    pub schema_version: String,
    pub enabled: bool,
    pub color: [f32; 3],
    pub kind: LightKind,
    pub layers: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cast_shadows: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LightKind {
    Directional {
        illuminance_lux: f32,
    },
    Point {
        intensity_candela: f32,
        range_m: f32,
    },
    Spot {
        intensity_candela: f32,
        range_m: f32,
        inner_half_angle_degrees: f32,
        outer_half_angle_degrees: f32,
    },
}
impl Light3D {
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "light3d.v1")?;
        finite(&self.color, "/color")?;
        names(&self.layers, "/layers")?;
        if self.cast_shadows && matches!(self.kind, LightKind::Point { .. }) {
            return Err(SpatialContractError::new(
                "point_shadow_unsupported",
                "/castShadows",
                "Point shadow projection is not supported.",
            ));
        }
        if self.cast_shadows
            && matches!(self.kind, LightKind::Spot { range_m, .. } if range_m<=0.05)
        {
            return Err(SpatialContractError::new(
                "spot_shadow_near_invalid",
                "/kind/rangeM",
                "Shadow range must exceed 0.05 metres.",
            ));
        }
        let (intensity, range) = match self.kind {
            LightKind::Directional { illuminance_lux } => (illuminance_lux, None),
            LightKind::Point {
                intensity_candela,
                range_m,
            } => (intensity_candela, Some(range_m)),
            LightKind::Spot {
                intensity_candela,
                range_m,
                inner_half_angle_degrees: inner,
                outer_half_angle_degrees: outer,
            } => {
                if !inner.is_finite()
                    || !outer.is_finite()
                    || inner < 0.
                    || inner >= outer
                    || outer >= 90.
                    || inner.to_radians().cos() <= outer.to_radians().cos()
                {
                    return Err(SpatialContractError::new(
                        "light_cone_invalid",
                        "/kind",
                        "Require 0 <= inner < outer < 90 degrees.",
                    ));
                }
                (intensity_candela, Some(range_m))
            }
        };
        if self.color.iter().any(|v| *v < 0.)
            || !intensity.is_finite()
            || intensity < 0.
            || self.color.iter().any(|v| !(*v * intensity).is_finite())
            || range.is_some_and(|v| !v.is_finite() || v <= 0. || v > 32768.)
        {
            return Err(SpatialContractError::new(
                "light_value_invalid",
                "/kind",
                "Nonnegative finite color/intensity and range in (0,32768] metres required.",
            ));
        }
        Ok(())
    }
}
