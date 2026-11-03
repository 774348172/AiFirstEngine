//! Accepted custom AUI extension state. DLL callbacks are dispatched by the native session owner.

use project_runtime_sdk::aui::UiNodeHandle;
use project_runtime_sdk::aui_extension::{
    UiExtensionChildInput, UiExtensionChildRect, UiExtensionConstraints, UiExtensionInput,
    UiExtensionMeasureOutput, UiExtensionPaintOutput, UiExtensionValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuiCustomNodeSpec {
    pub type_id: String,
    pub schema_version: u32,
    #[serde(default)]
    pub properties: BTreeMap<String, UiExtensionValue>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AuiCustomRuntimeOutput {
    pub measure: Option<UiExtensionMeasureOutput>,
    pub child_rects: Vec<UiExtensionChildRect>,
    pub paint: Option<UiExtensionPaintOutput>,
    pub measure_identity: Option<AuiCustomStageIdentity>,
    pub layout_identity: Option<AuiCustomStageIdentity>,
    pub paint_identity: Option<AuiCustomStageIdentity>,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuiCustomStageIdentity {
    pub properties: BTreeMap<String, UiExtensionValue>,
    pub input: UiExtensionInput,
    pub resource_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuiCustomFrameNode {
    pub node_id: String,
    pub constraints: Option<UiExtensionConstraints>,
    pub size: [f32; 2],
    pub children: Vec<(String, [f32; 2])>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AuiCustomFrame {
    pub document_id: String,
    pub resource_identity: String,
    pub nodes: Vec<AuiCustomFrameNode>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuiCustomFrameResult {
    pub changed: bool,
    pub stabilization_limit_exceeded: bool,
}

impl AuiCustomFrameNode {
    pub fn layout_input(
        &self,
        children: impl Fn(&str) -> Option<UiNodeHandle>,
    ) -> Option<UiExtensionInput> {
        Some(UiExtensionInput::Layout {
            size: self.size,
            children: self
                .children
                .iter()
                .map(|(node_id, measured_size)| {
                    Some(UiExtensionChildInput {
                        handle: children(node_id)?,
                        measured_size: *measured_size,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuiCustomRuntimeNode {
    pub handle: UiNodeHandle,
    pub node_id: String,
    pub spec: AuiCustomNodeSpec,
    pub rect: [f32; 4],
    pub children: Vec<UiNodeHandle>,
    pub runtime: AuiCustomRuntimeOutput,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuiCustomDirty {
    pub measure: bool,
    pub layout: bool,
    pub paint: bool,
}

impl AuiCustomDirty {
    pub const fn all() -> Self {
        Self {
            measure: true,
            layout: true,
            paint: true,
        }
    }
}
