use serde::{Deserialize, Serialize};

use super::SceneVisualOrderAuthoringModel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HierarchyModel {
    pub scene_id: Option<String>,
    pub roots: Vec<HierarchyNode>,
    pub selected_entity_id: Option<String>,
    pub authoring_view: HierarchyAuthoringView,
    pub visual_order: Option<SceneVisualOrderAuthoringModel>,
    pub source_domain: HierarchySourceDomain,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HierarchyNode {
    pub entity_id: String,
    pub label: String,
    pub alive: bool,
    pub children: Vec<HierarchyNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HierarchyAuthoringView {
    EntityTree,
    VisualOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HierarchySourceDomain {
    AuthoringScene,
    ActiveGameViewRuntime,
    OpenedRuntimePackage,
    Empty,
}

impl Default for HierarchySourceDomain {
    fn default() -> Self {
        Self::Empty
    }
}
