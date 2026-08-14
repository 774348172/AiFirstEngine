use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::runtime_asset::RuntimeAssetHandle;
use crate::runtime_instance_diagnostics::{InstanceDiagnostic, InstanceStage};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeInstanceId(pub u64);

impl fmt::Display for RuntimeInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeInstanceState {
    Allocating,
    Active,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeReportLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone)]
pub struct RuntimeSceneInstance {
    pub instance_id: RuntimeInstanceId,
    pub scene_asset_guid: String,
    pub scene_id: String,
    pub root_entities: Vec<RuntimeEntityId>,
    pub source_to_runtime_entity: BTreeMap<SourceEntityId, RuntimeEntityId>,
    pub source_to_world_entity: BTreeMap<SourceEntityId, SourceEntityId>,
    pub owned_asset_handles: Vec<RuntimeAssetHandle>,
    pub state: RuntimeInstanceState,
}

#[derive(Debug, Clone)]
pub struct RuntimePrefabInstance {
    pub instance_id: RuntimeInstanceId,
    pub prefab_asset_guid: String,
    pub root_entity: Option<RuntimeEntityId>,
    pub parent_entity: Option<SourceEntityId>,
    pub target_scene_instance: Option<RuntimeInstanceId>,
    pub source_to_runtime_entity: BTreeMap<SourceEntityId, RuntimeEntityId>,
    pub source_to_world_entity: BTreeMap<SourceEntityId, SourceEntityId>,
    pub owned_asset_handles: Vec<RuntimeAssetHandle>,
    pub state: RuntimeInstanceState,
}

#[derive(Debug, Clone)]
pub struct RuntimeInstantiateReport {
    pub request_id: u64,
    pub instance_id: Option<RuntimeInstanceId>,
    pub asset_ref: String,
    pub stage: InstanceStage,
    pub created_entity_count: usize,
    pub loaded_asset_count: usize,
    pub remapped_reference_count: usize,
    pub committed: bool,
    pub world_changed: bool,
    pub report_level: RuntimeReportLevel,
    pub diagnostics: Vec<InstanceDiagnostic>,
    pub source_to_runtime_entity_debug: Vec<(String, String)>,
}

impl RuntimeInstantiateReport {
    pub fn new(request_id: u64, asset_ref: impl Into<String>) -> Self {
        Self {
            request_id,
            instance_id: None,
            asset_ref: asset_ref.into(),
            stage: InstanceStage::ResolveAssets,
            created_entity_count: 0,
            loaded_asset_count: 0,
            remapped_reference_count: 0,
            committed: false,
            world_changed: false,
            report_level: RuntimeReportLevel::Off,
            diagnostics: Vec::new(),
            source_to_runtime_entity_debug: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity
                == crate::runtime_instance_diagnostics::InstanceDiagnosticSeverity::Error
        })
    }

    pub fn with_report_level(mut self, report_level: RuntimeReportLevel) -> Self {
        self.report_level = report_level;
        self
    }
}

pub type SceneInstantiateReport = RuntimeInstantiateReport;
pub type PrefabInstantiateReport = RuntimeInstantiateReport;
