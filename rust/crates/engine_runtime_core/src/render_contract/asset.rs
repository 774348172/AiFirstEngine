use super::{ContractError, ContractErrorCode, ProducerStage, ValidateRenderContract};
use crate::runtime_asset::{RuntimeAssetIndex, RuntimeAssetRecord, RuntimeAssetResolveError};
use crate::runtime_package::RuntimeAssetRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetKind {
    Environment,
    Texture,
    Material,
    Mesh,
    Font,
}

impl RenderAssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Texture => "texture",
            Self::Material => "material",
            Self::Mesh => "mesh",
            Self::Font => "font",
        }
    }
}

// Serde remote definition supplies strict decoding only for the NEW input surface.
// Storage, identity, old JSON keys and resolution still belong to RuntimeAssetRef.
#[derive(Serialize, Deserialize)]
#[serde(remote = "RuntimeAssetRef", deny_unknown_fields)]
struct StrictRuntimeAssetRef {
    id: String,
    #[serde(rename = "type")]
    asset_type: String,
    #[serde(default)]
    guid: Option<String>,
    #[serde(default)]
    sub_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderAssetReference {
    pub expected_kind: RenderAssetKind,
    #[serde(with = "StrictRuntimeAssetRef")]
    pub reference: RuntimeAssetRef,
}

#[derive(Debug)]
pub enum RenderAssetResolution<'a> {
    Unresolved,
    /// The authoritative record was found. This does NOT imply uploaded/ready geometry.
    Resolved(&'a RuntimeAssetRecord),
}

impl ValidateRenderContract for RenderAssetReference {
    fn validate(&self) -> Result<(), ContractError> {
        if self.reference.asset_type != self.expected_kind.as_str() {
            return Err(ContractError::new(
                ContractErrorCode::AssetTypeMismatch,
                "reference.type",
            ));
        }
        if self.reference.id.trim().is_empty()
            && self
                .reference
                .guid
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(ContractError::new(
                ContractErrorCode::AssetMissing,
                "reference",
            ));
        }
        Ok(())
    }
}

impl RenderAssetReference {
    pub fn to_legacy(&self) -> Result<RuntimeAssetRef, ContractError> {
        self.validate()?;
        Ok(self.reference.clone())
    }

    pub fn resolve<'a>(
        &self,
        index: Option<&'a RuntimeAssetIndex>,
    ) -> Result<RenderAssetResolution<'a>, ContractError> {
        self.validate()?;
        let Some(index) = index else {
            return Ok(RenderAssetResolution::Unresolved);
        };
        let record = index.resolve(&self.reference).map_err(|error| {
            ContractError::new(
                match error {
                    RuntimeAssetResolveError::MissingAssetRef => ContractErrorCode::AssetMissing,
                    RuntimeAssetResolveError::TypeMismatch { .. } => {
                        ContractErrorCode::AssetTypeMismatch
                    }
                    RuntimeAssetResolveError::SubAssetMissing => {
                        ContractErrorCode::AssetSubAssetMissing
                    }
                },
                "reference",
            )
            .at_stage(ProducerStage::AssetResolution)
        })?;
        if !self.reference.id.is_empty() && self.reference.id != record.asset_id {
            return Err(ContractError::new(
                ContractErrorCode::AssetIdentityConflict,
                "reference.id",
            )
            .at_stage(ProducerStage::AssetResolution));
        }
        Ok(RenderAssetResolution::Resolved(record))
    }
}
