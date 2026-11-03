//! 359-A0 spatial wire contracts. Validation is not hydration or rendering readiness.
//! Right handed, Y up, metres, camera -Z; column vectors and clip depth [0, 1].
use crate::render_contract::{
    Projection, RenderAssetKind, RenderAssetReference, ValidateRenderContract,
};
use crate::runtime_package::{RuntimeAssetRef, RuntimeEntity, RuntimePrefabData, RuntimeScene};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(test, path = "spatial_contract/light.rs")]
mod light;
pub use light::*;
#[cfg_attr(test, path = "spatial_contract/document.rs")]
mod document;
#[cfg_attr(test, path = "spatial_contract/json.rs")]
mod json;
#[cfg_attr(test, path = "spatial_contract/wire.rs")]
mod wire;
#[cfg_attr(test, path = "spatial_contract/world_position.rs")]
mod world_position;
pub use document::*;
pub use json::strict_json;
pub use world_position::{CoordinateDomain, PersistentEntityIdentity, WorldPosition};

pub const QUATERNION_LENGTH_TOLERANCE: f64 = 1e-4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialContractError {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl SpatialContractError {
    pub fn new(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self {
            code: format!("three_d.{code}"),
            field_path: path.into(),
            message: message.into(),
        }
    }
    pub fn at(mut self, prefix: &str) -> Self {
        self.field_path = format!("{prefix}{}", self.field_path);
        self
    }
}
impl std::fmt::Display for SpatialContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.field_path, self.message)
    }
}
impl std::error::Error for SpatialContractError {}
pub type ContractResult<T> = Result<T, SpatialContractError>;

pub fn decode<T: serde::de::DeserializeOwned>(value: Value, path: &str) -> ContractResult<T> {
    serde_json::from_value(value)
        .map_err(|e| SpatialContractError::new("schema_invalid", path, e.to_string()))
}
pub fn version(actual: &str, expected: &str) -> ContractResult<()> {
    if actual != expected {
        return Err(SpatialContractError::new(
            "schema_unsupported",
            "/schemaVersion",
            format!("Expected {expected}, got {actual}."),
        ));
    }
    Ok(())
}
pub fn finite(values: &[f32], path: &str) -> ContractResult<()> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(SpatialContractError::new(
            "non_finite",
            path,
            "All numbers must be finite.",
        ));
    }
    Ok(())
}
pub fn names(values: &[String], path: &str) -> ContractResult<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || !seen.insert(value) {
            return Err(SpatialContractError::new(
                "name_invalid",
                path,
                "Names must be nonempty and unique.",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Rotation3D {
    EulerDegrees { xyz: [f32; 3] },
    Quaternion { xyzw: [f32; 4] },
}

pub fn canonical_quaternion(xyzw: [f32; 4]) -> ContractResult<[f32; 4]> {
    finite(&xyzw, "/rotation/xyzw")?;
    let length = xyzw
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if length == 0.0 || (length - 1.0).abs() > QUATERNION_LENGTH_TOLERANCE {
        return Err(SpatialContractError::new(
            "quaternion_invalid",
            "/rotation/xyzw",
            "Quaternion length must be within 1e-4 of one.",
        ));
    }
    let mut q = xyzw.map(|v| (f64::from(v) / length) as f32);
    let sign = [q[3], q[0], q[1], q[2]]
        .into_iter()
        .find(|v| *v != 0.0)
        .unwrap_or(1.0);
    if sign < 0.0 {
        q = q.map(|v| -v);
    }
    Ok(q.map(|v| if v == 0.0 { 0.0 } else { v }))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transform3D {
    pub schema_version: String,
    pub translation: [f32; 3],
    pub rotation: Rotation3D,
    pub scale: [f32; 3],
}

/// Package transform contains only canonical xyzw, never a second Euler authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalTransform3D {
    pub translation: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

impl Transform3D {
    pub fn identity() -> Self {
        Self {
            schema_version: "transform3d.v1".into(),
            translation: [0.0; 3],
            rotation: Rotation3D::EulerDegrees { xyz: [0.0; 3] },
            scale: [1.0; 3],
        }
    }
    pub fn canonicalize(&self) -> ContractResult<CanonicalTransform3D> {
        version(&self.schema_version, "transform3d.v1")?;
        let rotation_xyzw = match self.rotation {
            Rotation3D::Quaternion { xyzw } => canonical_quaternion(xyzw)?,
            Rotation3D::EulerDegrees { xyz } => {
                finite(&xyz, "/rotation/xyz")?;
                // qz * qy * qx: apply X then Y then Z to column vectors.
                let [(sx, cx), (sy, cy), (sz, cz)] =
                    xyz.map(|v| (f64::from(v).rem_euclid(360.0).to_radians() * 0.5).sin_cos());
                canonical_quaternion([
                    (sx * cy * cz - cx * sy * sz) as f32,
                    (cx * sy * cz + sx * cy * sz) as f32,
                    (cx * cy * sz - sx * sy * cz) as f32,
                    (cx * cy * cz + sx * sy * sz) as f32,
                ])?
            }
        };
        let canonical = CanonicalTransform3D {
            translation: self.translation,
            rotation_xyzw,
            scale: self.scale,
        };
        canonical.validate()?;
        Ok(canonical)
    }
}
#[cfg(test)]
std::thread_local! { pub(crate) static CANONICAL_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

impl CanonicalTransform3D {
    /// Validate package values without renormalizing rounded f32 components.
    /// Repeated normalization can change a low component by one ULP on each load.
    pub fn validate(&self) -> ContractResult<()> {
        #[cfg(test)]
        CANONICAL_VALIDATIONS.with(|n| n.set(n.get() + 1));
        finite(&self.translation, "/translation")?;
        finite(&self.scale, "/scale")?;
        let determinant = self.scale.iter().map(|v| f64::from(*v)).product::<f64>() as f32;
        if self
            .scale
            .iter()
            .any(|v| *v == 0.0 || !(1.0 / v).is_finite())
            || !determinant.is_finite()
            || determinant == 0.0
        {
            return Err(SpatialContractError::new(
                "scale_singular",
                "/scale",
                "Scale and its inverse/determinant must be representable and nonzero.",
            ));
        }
        finite(&self.rotation_xyzw, "/rotationXyzw")?;
        let q = self.rotation_xyzw;
        let length = q.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
        let sign = [q[3], q[0], q[1], q[2]]
            .into_iter()
            .find(|v| *v != 0.0)
            .unwrap_or(0.0);
        if (length - 1.0).abs() > 2.0 * f64::from(f32::EPSILON)
            || sign <= 0.0
            || q.iter().any(|v| *v == 0.0 && v.is_sign_negative())
        {
            return Err(SpatialContractError::new(
                "quaternion_not_canonical",
                "/rotationXyzw",
                "Package quaternion must already be normalized to f32 precision with canonical sign and positive zero. Rebuild from authored Transform3D.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visibility3D {
    pub schema_version: String,
    pub visible: bool,
}

// Keep the existing AssetRef wire keys and resolution owner, with strict NEW decoding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpatialAssetRef(#[serde(with = "StrictAssetRef")] pub RuntimeAssetRef);
#[derive(Serialize, Deserialize)]
#[serde(remote = "RuntimeAssetRef", deny_unknown_fields)]
struct StrictAssetRef {
    id: String,
    #[serde(rename = "type")]
    asset_type: String,
    #[serde(default)]
    guid: Option<String>,
    #[serde(default)]
    sub_asset: Option<String>,
}
impl SpatialAssetRef {
    pub fn validate(&self, expected: RenderAssetKind, path: &str) -> ContractResult<()> {
        RenderAssetReference {
            expected_kind: expected,
            reference: self.0.clone(),
        }
        .validate()
        .map_err(|e| {
            SpatialContractError::new("asset_reference_invalid", path, format!("{e:?}"))
        })?;
        if self.0.guid.as_ref().is_some_and(|v| v.trim().is_empty())
            || self
                .0
                .sub_asset
                .as_ref()
                .is_some_and(|v| v.trim().is_empty())
        {
            return Err(SpatialContractError::new(
                "asset_reference_invalid",
                path,
                "GUID/sub_asset cannot be blank when present.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshRenderer3D {
    pub schema_version: String,
    pub mesh: SpatialAssetRef,
    pub materials: BTreeMap<String, SpatialAssetRef>,
    pub visible: bool,
    pub layers: Vec<String>,
}
impl MeshRenderer3D {
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "mesh-renderer3d.v1")?;
        self.mesh.validate(RenderAssetKind::Mesh, "/mesh")?;
        names(&self.layers, "/layers")?;
        names(
            &self.materials.keys().cloned().collect::<Vec<_>>(),
            "/materials",
        )?;
        for (slot, reference) in &self.materials {
            reference.validate(
                RenderAssetKind::Material,
                &format!("/materials/{}", pointer_token(slot)),
            )?;
        }
        Ok(())
    }
    pub fn validate_slots(&self, used_slots: &[String]) -> ContractResult<()> {
        self.validate()?;
        names(used_slots, "/materialSlots")?;
        if self.materials.keys().collect::<BTreeSet<_>>()
            != used_slots.iter().collect::<BTreeSet<_>>()
        {
            return Err(SpatialContractError::new(
                "material_slots_mismatch",
                "/materials",
                "Bindings must exactly match named mesh slots.",
            ));
        }
        Ok(())
    }
}
pub fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CameraViewport {
    FullWindow,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Camera3D {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_pipeline: Option<crate::color_pipeline::ColorPipeline3D>,

    pub schema_version: String,
    pub projection: Projection,
    pub viewport: CameraViewport,
    pub clear_color: [f32; 4],
    pub layers: Vec<String>,
    pub enabled: bool,
}
impl Camera3D {
    pub fn perspective_template() -> Self {
        Self {
            schema_version: "camera3d.v1".into(),
            color_pipeline: None,
            projection: Projection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            },
            viewport: CameraViewport::FullWindow,
            clear_color: [0.0, 0.0, 0.0, 1.0],
            layers: vec!["world".into()],
            enabled: true,
        }
    }
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "camera3d.v1")?;
        if let Some(pipeline) = &self.color_pipeline {
            pipeline
                .validate_clear(Some(self.clear_color))
                .and_then(|_| pipeline.validate())
                .map_err(|e| {
                    SpatialContractError::new(
                        "color_pipeline_invalid",
                        "/colorPipeline",
                        e.to_string(),
                    )
                })?;
        }
        if matches!(self.projection, Projection::LegacyMatrix { .. }) {
            return Err(SpatialContractError::new(
                "projection_invalid",
                "/projection",
                "Camera3D requires perspective or orthographic parameters.",
            ));
        }
        self.projection.validate().map_err(|e| {
            SpatialContractError::new("projection_invalid", "/projection", format!("{e:?}"))
        })?;
        finite(&self.clear_color, "/clearColor")?;
        names(&self.layers, "/layers")
    }
}
