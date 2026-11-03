//! Neutral render contract v1. This is not a RuntimePackage or SDK schema upgrade.
//! Legacy conversion is explicit and never infers 3D execution from a valid contract.

#[cfg_attr(test, path = "render_contract/asset.rs")]
mod asset;
#[cfg_attr(test, path = "render_contract/binding.rs")]
mod binding;
#[cfg_attr(test, path = "render_contract/camera.rs")]
mod camera;
#[cfg_attr(test, path = "render_contract/cooked.rs")]
mod cooked;
#[cfg_attr(test, path = "render_contract/diagnostic.rs")]
mod diagnostic;
#[cfg_attr(test, path = "render_contract/view.rs")]
mod view;

pub use asset::*;
pub use binding::*;
pub use camera::*;
pub use cooked::*;
pub use diagnostic::*;
pub use view::*;

use crate::components::Transform;
use crate::math::Vec3;
use crate::render_state::Bounds;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderContractVersion {
    #[serde(rename = "render-contract.v1")]
    V1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderContract<T> {
    pub schema_version: RenderContractVersion,
    pub data: T,
}

impl<T> RenderContract<T> {
    pub fn new(data: T) -> Self {
        Self {
            schema_version: RenderContractVersion::V1,
            data,
        }
    }
}

impl<T: DeserializeOwned + ValidateRenderContract> RenderContract<T> {
    /// Only new contract input uses this strict decoder. Legacy decoders are unchanged.
    pub fn decode(json: &str) -> Result<Self, ContractDecodeError> {
        let value: Self = serde_json::from_str(json).map_err(ContractDecodeError::Schema)?;
        value
            .data
            .validate()
            .map_err(ContractDecodeError::Invalid)?;
        Ok(value)
    }
}

#[derive(Debug)]
pub enum ContractDecodeError {
    Schema(serde_json::Error),
    Invalid(ContractError),
}

impl ContractDecodeError {
    pub fn compact(&self) -> ContractError {
        match self {
            Self::Schema(_) => ContractError::new(ContractErrorCode::InvalidSchema, "$"),
            Self::Invalid(error) => *error,
        }
    }
}

impl std::fmt::Display for ContractDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(error) => error.fmt(f),
            Self::Invalid(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ContractDecodeError {}

pub trait ValidateRenderContract {
    fn validate(&self) -> Result<(), ContractError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderDomain {
    World2d,
    World3d,
    Ui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionSupport {
    LegacyCompatible,
    /// A native geometry consumer exists; pose, resources and backend remain required.
    NativeGeometry,
    ContractOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum Evaluation<T> {
    NotEvaluated,
    Known(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoordinateConvention {
    Legacy2d,
    World3dRightHandedYUpMetersCameraNegativeZ,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransformMetadata {
    pub convention: CoordinateConvention,
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
    pub world_matrix: Evaluation<[f32; 16]>,
    pub previous_world_matrix: Evaluation<[f32; 16]>,
}

impl TransformMetadata {
    pub fn from_legacy(transform: &Transform) -> Self {
        Self {
            convention: CoordinateConvention::Legacy2d,
            position: vector(transform.local_position),
            rotation_degrees: vector(transform.local_rotation),
            scale: vector(transform.local_scale),
            world_matrix: Evaluation::NotEvaluated,
            previous_world_matrix: Evaluation::NotEvaluated,
        }
    }

    /// Preserves old numeric values; strict new-input validation is a separate operation.
    pub fn to_legacy(&self) -> Result<Transform, ContractError> {
        if self.convention != CoordinateConvention::Legacy2d
            || !matches!(self.world_matrix, Evaluation::NotEvaluated)
            || !matches!(self.previous_world_matrix, Evaluation::NotEvaluated)
        {
            return Err(unsupported("transform"));
        }
        Ok(Transform {
            local_position: vec3(self.position),
            local_rotation: vec3(self.rotation_degrees),
            local_scale: vec3(self.scale),
        })
    }
}

impl ValidateRenderContract for TransformMetadata {
    fn validate(&self) -> Result<(), ContractError> {
        finite(&self.position, "position")?;
        finite(&self.rotation_degrees, "rotationDegrees")?;
        finite(&self.scale, "scale")?;
        if let Evaluation::Known(matrix) = &self.world_matrix {
            finite(matrix, "worldMatrix")?;
        }
        if let Evaluation::Known(matrix) = &self.previous_world_matrix {
            finite(matrix, "previousWorldMatrix")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoundsQuality {
    LegacyApproximation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoundsSpace {
    LegacyTransform,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundsMetadata {
    pub center: [f32; 3],
    pub extents: [f32; 3],
    pub quality: BoundsQuality,
    pub space: BoundsSpace,
}

impl BoundsMetadata {
    pub fn from_legacy(bounds: &Bounds) -> Self {
        Self {
            center: vector(bounds.center),
            extents: vector(bounds.extents),
            quality: BoundsQuality::LegacyApproximation,
            space: BoundsSpace::LegacyTransform,
        }
    }
}

impl ValidateRenderContract for BoundsMetadata {
    fn validate(&self) -> Result<(), ContractError> {
        finite(&self.center, "center")?;
        finite(&self.extents, "extents")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visibility {
    pub authored: bool,
    pub inherited: Evaluation<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CullingDecision {
    NotEvaluated,
    Included,
    LayerExcluded,
    DomainExcluded,
    FrustumExcluded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewVisibility {
    pub view_id: u64,
    pub instance_id: u64,
    pub visibility: Evaluation<bool>,
    pub culling: CullingDecision,
}

impl ViewVisibility {
    pub fn key(&self) -> (u64, u64) {
        (self.view_id, self.instance_id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LayerSelection {
    Legacy { mask: Option<String> },
    Named { names: Vec<String> },
}

pub(crate) fn validate_named_layers(names: &[String]) -> Result<(), ContractError> {
    let mut unique = std::collections::BTreeSet::new();
    if names
        .iter()
        .any(|name| name.trim().is_empty() || !unique.insert(name))
    {
        return Err(ContractError::new(
            ContractErrorCode::InvalidLayers,
            "layers",
        ));
    }
    Ok(())
}

impl ValidateRenderContract for LayerSelection {
    fn validate(&self) -> Result<(), ContractError> {
        if let Self::Named { names } = self {
            validate_named_layers(names)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RenderPolicy {
    /// Includes legacy zero-Z fallback, quantization and proxy-id tie-break semantics.
    LegacySprite {
        sorting_layer: i16,
        order_in_layer: i32,
        sort_z: f32,
    },
    Opaque,
    Transparent,
    UiComposition,
}

impl ValidateRenderContract for RenderPolicy {
    fn validate(&self) -> Result<(), ContractError> {
        if let Self::LegacySprite { sort_z, .. } = self {
            finite(&[*sort_z], "sortZ")?;
        }
        Ok(())
    }
}

/// Provenance remains an explicit attachment, never a silently dropped legacy field.
#[derive(Debug)]
pub struct LegacyConversion<'a, T> {
    pub payload: T,
    pub provenance: &'a Provenance,
}

pub(crate) fn finite(values: &[f32], field: &'static str) -> Result<(), ContractError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(ContractError::new(ContractErrorCode::NonFinite, field))
    }
}

pub(crate) fn unsupported(field: &'static str) -> ContractError {
    ContractError::new(ContractErrorCode::UnsupportedLegacyConversion, field)
        .at_stage(ProducerStage::RenderProjection)
}

fn vector(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}
fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3 {
        x: value[0],
        y: value[1],
        z: value[2],
    }
}
