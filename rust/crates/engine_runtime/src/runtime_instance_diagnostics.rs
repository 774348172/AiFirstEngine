use crate::ids::{RuntimeEntityId, SourceEntityId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStage {
    ResolveAssets,
    ValidateInput,
    PrepareEntities,
    CommitEntities,
    AllocateEntities,
    AttachComponents,
    RemapReferences,
    Activate,
    Release,
}

impl InstanceStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstanceStage::ResolveAssets => "ResolveAssets",
            InstanceStage::ValidateInput => "ValidateInput",
            InstanceStage::PrepareEntities => "PrepareEntities",
            InstanceStage::CommitEntities => "CommitEntities",
            InstanceStage::AllocateEntities => "AllocateEntities",
            InstanceStage::AttachComponents => "AttachComponents",
            InstanceStage::RemapReferences => "RemapReferences",
            InstanceStage::Activate => "Activate",
            InstanceStage::Release => "Release",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDiagnostic {
    pub severity: InstanceDiagnosticSeverity,
    pub kind: String,
    pub message: String,
    pub stage: InstanceStage,
    pub asset_guid: Option<String>,
    pub source_entity_id: Option<SourceEntityId>,
    pub runtime_entity_id: Option<RuntimeEntityId>,
    pub suggested_fix: Option<String>,
}

impl InstanceDiagnostic {
    pub fn error(
        kind: impl Into<String>,
        message: impl Into<String>,
        stage: InstanceStage,
    ) -> Self {
        Self {
            severity: InstanceDiagnosticSeverity::Error,
            kind: kind.into(),
            message: message.into(),
            stage,
            asset_guid: None,
            source_entity_id: None,
            runtime_entity_id: None,
            suggested_fix: None,
        }
    }

    pub fn with_asset_guid(mut self, asset_guid: impl Into<String>) -> Self {
        self.asset_guid = Some(asset_guid.into());
        self
    }

    pub fn with_source_entity_id(mut self, source_entity_id: SourceEntityId) -> Self {
        self.source_entity_id = Some(source_entity_id);
        self
    }

    pub fn with_runtime_entity_id(mut self, runtime_entity_id: RuntimeEntityId) -> Self {
        self.runtime_entity_id = Some(runtime_entity_id);
        self
    }

    pub fn with_suggested_fix(mut self, suggested_fix: impl Into<String>) -> Self {
        self.suggested_fix = Some(suggested_fix.into());
        self
    }
}
