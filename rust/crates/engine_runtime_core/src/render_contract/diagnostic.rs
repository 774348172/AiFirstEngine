use super::{RenderContractVersion, RenderDomain};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContractErrorCode {
    InvalidSchema,
    AssetMissing,
    AssetTypeMismatch,
    AssetSubAssetMissing,
    AssetIdentityConflict,
    NonFinite,
    InvalidProjection,
    InvalidViewport,
    InvalidLayers,
    InvalidDomain,
    UnsupportedLegacyConversion,
    InvalidAssetPayload,
    UnsupportedMaterialParameter,
    AssetDigestMismatch,
    MissingCookedPayload,
    ResourceTypeMismatch,
    InvalidBinding,
    StaleResourceHandle,
    ResourceNotReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractError {
    pub code: ContractErrorCode,
    pub field: &'static str,
    pub stage: ProducerStage,
}

impl ContractError {
    pub fn new(code: ContractErrorCode, field: &'static str) -> Self {
        Self {
            code,
            field,
            stage: ProducerStage::ContractValidation,
        }
    }

    pub fn at_stage(mut self, stage: ProducerStage) -> Self {
        self.stage = stage;
        self
    }

    fn advice(self) -> (&'static str, &'static str) {
        match self.code {
            ContractErrorCode::InvalidSchema => (
                "render-contract.v1 with declared tags and fields",
                "correct the schema version, field or tag reported by the decoder",
            ),
            ContractErrorCode::AssetMissing => (
                "existing asset identity",
                "bind an asset present in the RuntimeAssetIndex",
            ),
            ContractErrorCode::AssetTypeMismatch => (
                "matching expected and actual asset kinds",
                "select an asset of the required type",
            ),
            ContractErrorCode::AssetSubAssetMissing => (
                "existing sub_asset",
                "use a subasset declared by the authoritative record",
            ),
            ContractErrorCode::AssetIdentityConflict => (
                "id and guid identify the same record",
                "correct the conflicting reference; do not replace the asset record",
            ),
            ContractErrorCode::NonFinite => (
                "finite numeric values",
                "replace NaN or infinity in the indicated field",
            ),
            ContractErrorCode::InvalidProjection => (
                "positive size or 0<fov<180 and 0<near<far for perspective",
                "correct the camera parameters",
            ),
            ContractErrorCode::InvalidViewport => (
                "nonzero width/height and nonoverflowing viewport extent",
                "provide a valid active viewport",
            ),
            ContractErrorCode::InvalidLayers => (
                "unique nonempty named layers",
                "remove duplicate or empty layer names",
            ),
            ContractErrorCode::InvalidDomain => (
                "consistent domain, coordinate convention and policy",
                "use the policy and coordinates belonging to this domain",
            ),
            ContractErrorCode::UnsupportedLegacyConversion => (
                "lossless legacy-compatible subset",
                "retain the typed contract; wait for a supporting consumer instead of discarding fields",
            ),
            ContractErrorCode::InvalidAssetPayload => (
                "validated native render data",
                "correct the indicated payload field and recook the asset",
            ),
            ContractErrorCode::UnsupportedMaterialParameter => (
                "parameters declared by the selected material preset",
                "use supported named parameters with their declared types",
            ),
            ContractErrorCode::AssetDigestMismatch => (
                "payload matching runtime and cooked manifest digests",
                "recook and reload the same package revision",
            ),
            ContractErrorCode::MissingCookedPayload => (
                "real cooked asset bytes",
                "cook the asset; metadata cannot substitute geometry or material data",
            ),
            ContractErrorCode::ResourceTypeMismatch => (
                "resource kind matching the typed handle and slot",
                "bind a resource of the declared kind",
            ),
            ContractErrorCode::InvalidBinding => (
                "unique named slots with matching resource types",
                "correct the binding slots without dropping required resources",
            ),
            ContractErrorCode::StaleResourceHandle => (
                "current resident handle generation",
                "prepare the current asset revision and replace the stale binding",
            ),
            ContractErrorCode::ResourceNotReady => (
                "resident resource",
                "retry after resource preparation or upload budget is available",
            ),
        }
    }
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at {} ({:?})", self.code, self.field, self.stage)
    }
}

impl std::error::Error for ContractError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProducerStage {
    ContractValidation,
    AssetResolution,
    RenderProjection,
    AssetDecode,
    ResourcePreparation,
    AssetBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceAvailability {
    LegacyContextOnly,
    InputContextNotSupplied,
    SuppliedContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub source_entity_id: Option<String>,
    pub source_component: Option<String>,
    pub source_path: Option<String>,
    pub project_revision: Option<String>,
    pub operation_id: Option<String>,
    pub asset_id: Option<String>,
    pub content_digest: Option<String>,
    pub producer_stage: ProducerStage,
    pub availability: SourceAvailability,
}

impl Provenance {
    pub fn unspecified(stage: ProducerStage) -> Self {
        Self {
            availability: SourceAvailability::InputContextNotSupplied,
            ..Self::legacy(stage)
        }
    }

    pub fn legacy_sprite(entity: &str) -> Self {
        Self {
            source_entity_id: Some(entity.into()),
            source_component: Some("SpriteRenderer2D".into()),
            ..Self::legacy(ProducerStage::RenderProjection)
        }
    }

    pub fn legacy(stage: ProducerStage) -> Self {
        Self {
            source_entity_id: None,
            source_component: None,
            source_path: None,
            project_revision: None,
            operation_id: None,
            asset_id: None,
            content_digest: None,
            producer_stage: stage,
            availability: SourceAvailability::LegacyContextOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FallbackStatus {
    Accepted,
    Degraded,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Retryability {
    AfterSourceChange,
    RequiresConsumer,
    AfterResourcePreparation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractDiagnostic {
    pub schema_version: RenderContractVersion,
    pub code: ContractErrorCode,
    pub field: &'static str,
    pub stage: ProducerStage,
    pub domain: RenderDomain,
    pub provenance: Provenance,
    pub expected: &'static str,
    pub actual: String,
    pub dependency_chain: Vec<String>,
    pub fallback: FallbackStatus,
    pub retryability: Retryability,
    pub next_action: &'static str,
}

#[derive(Debug, Default)]
pub struct DiagnosticDetails {
    pub actual: String,
    pub dependency_chain: Vec<String>,
}

pub const SUMMARY_LIMIT: usize = 16;
pub const SUMMARY_DEPENDENCY_LIMIT: usize = 8;
const SUMMARY_TEXT_LIMIT: usize = 256;

#[derive(Debug, Default)]
pub struct ContractDiagnostics {
    pub level: DiagnosticLevel,
    pub entries: Vec<ContractDiagnostic>,
}

impl ContractDiagnostics {
    pub fn new(level: DiagnosticLevel) -> Self {
        Self {
            level,
            entries: Vec::new(),
        }
    }

    /// The caller keeps the compact Result regardless of report mode. No closure,
    /// provenance cloning, JSON or strings are materialized in the default Off path.
    pub fn record(
        &mut self,
        error: ContractError,
        domain: RenderDomain,
        provenance: &Provenance,
        details: impl FnOnce() -> DiagnosticDetails,
    ) {
        if self.level == DiagnosticLevel::Off
            || (self.level == DiagnosticLevel::Summary && self.entries.len() >= SUMMARY_LIMIT)
        {
            return;
        }
        let mut details = details();
        if self.level == DiagnosticLevel::Summary {
            limit_text(&mut details.actual);
            details.dependency_chain.truncate(SUMMARY_DEPENDENCY_LIMIT);
            for item in &mut details.dependency_chain {
                limit_text(item);
            }
        }
        let (expected, next_action) = error.advice();
        self.entries.push(ContractDiagnostic {
            schema_version: RenderContractVersion::V1,
            code: error.code,
            field: error.field,
            stage: error.stage,
            domain,
            provenance: provenance.clone(),
            expected,
            actual: details.actual,
            dependency_chain: details.dependency_chain,
            fallback: FallbackStatus::Unsupported,
            retryability: match error.code {
                ContractErrorCode::UnsupportedLegacyConversion => Retryability::RequiresConsumer,
                ContractErrorCode::StaleResourceHandle | ContractErrorCode::ResourceNotReady => {
                    Retryability::AfterResourcePreparation
                }
                _ => Retryability::AfterSourceChange,
            },
            next_action,
        });
    }
}

fn limit_text(value: &mut String) {
    if let Some((index, _)) = value.char_indices().nth(SUMMARY_TEXT_LIMIT) {
        value.truncate(index);
    }
}
