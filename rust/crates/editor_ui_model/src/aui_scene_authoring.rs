use serde::{Deserialize, Serialize};

pub const AUI_SCENE_UNIFIED_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "aui-scene-unified-authoring-report.v1";
pub const SCENE_VISUAL_ORDER_AUTHORING_MODEL_SCHEMA_VERSION: &str =
    "scene-visual-order-authoring-model.v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiAuthoringVec2 {
    pub x: f32,
    pub y: f32,
}

impl AuiAuthoringVec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuiSourceRect {
    pub anchor_min: AuiAuthoringVec2,
    pub anchor_max: AuiAuthoringVec2,
    pub offset_min: AuiAuthoringVec2,
    pub offset_max: AuiAuthoringVec2,
    pub pivot: AuiAuthoringVec2,
    pub size: AuiAuthoringVec2,
}

impl AuiSourceRect {
    pub fn fixed_position(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            anchor_min: AuiAuthoringVec2::new(0.0, 0.0),
            anchor_max: AuiAuthoringVec2::new(0.0, 0.0),
            offset_min: AuiAuthoringVec2::new(x, y),
            offset_max: AuiAuthoringVec2::new(0.0, 0.0),
            pivot: AuiAuthoringVec2::new(0.0, 0.0),
            size: AuiAuthoringVec2::new(width, height),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiComputedAuthoringRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl AuiComputedAuthoringRect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiSceneViewProjection {
    Perspective,
    Orthographic2D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiNodeAuthoringProxy {
    pub document_path: String,
    pub document_id: String,
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub source_rect: AuiSourceRect,
    pub rect: AuiComputedAuthoringRect,
    pub visible: bool,
    pub interactable: bool,
    pub binding_count: usize,
    pub action_count: usize,
    pub selectable: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SceneVisualOrderRenderSpace {
    BeforeWorld,
    ScreenOverlay,
    Modal,
}

impl SceneVisualOrderRenderSpace {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BeforeWorld => "BeforeWorld",
            Self::ScreenOverlay => "ScreenOverlay",
            Self::Modal => "Modal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneVisualOrderTargetKind {
    SceneEntity,
    AuiCanvas,
    AuiLayerGroup,
    AuiNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualOrderKey {
    pub render_space: SceneVisualOrderRenderSpace,
    pub layer: i32,
    pub order: i32,
    pub local_order: i32,
}

impl VisualOrderKey {
    pub const fn before_world(layer: i32, order: i32, local_order: i32) -> Self {
        Self {
            render_space: SceneVisualOrderRenderSpace::BeforeWorld,
            layer,
            order,
            local_order,
        }
    }

    pub const fn screen_overlay(layer: i32, order: i32, local_order: i32) -> Self {
        Self {
            render_space: SceneVisualOrderRenderSpace::ScreenOverlay,
            layer,
            order,
            local_order,
        }
    }

    pub const fn modal(layer: i32, order: i32, local_order: i32) -> Self {
        Self {
            render_space: SceneVisualOrderRenderSpace::Modal,
            layer,
            order,
            local_order,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualOrderIntentRelation {
    None,
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualOrderIntent {
    pub relation: VisualOrderIntentRelation,
    pub target_kind: Option<SceneVisualOrderTargetKind>,
    pub target_ref: Option<String>,
    pub reason: Option<String>,
}

impl VisualOrderIntent {
    pub fn none() -> Self {
        Self {
            relation: VisualOrderIntentRelation::None,
            target_kind: None,
            target_ref: None,
            reason: None,
        }
    }

    pub fn after(
        target_kind: SceneVisualOrderTargetKind,
        target_ref: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            relation: VisualOrderIntentRelation::After,
            target_kind: Some(target_kind),
            target_ref: Some(target_ref.into()),
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneVisualOrderAuthoringEntry {
    pub entry_id: String,
    pub display_name: String,
    pub target_kind: SceneVisualOrderTargetKind,
    pub target_ref: String,
    pub parent_entry_id: Option<String>,
    pub visual_order_key: VisualOrderKey,
    pub visual_order_intent: VisualOrderIntent,
    pub runtime_supported: bool,
    pub runtime_support_reason: String,
    pub can_reorder: bool,
    pub reorder_scope: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneVisualOrderAuthoringModel {
    pub schema_version: String,
    pub scene_path: Option<String>,
    pub entries: Vec<SceneVisualOrderAuthoringEntry>,
    pub default_view_is_visual_order: bool,
    pub debug_bucket_view_available: bool,
    pub diagnostics: Vec<String>,
}

impl SceneVisualOrderAuthoringModel {
    pub fn empty(scene_path: Option<String>) -> Self {
        Self {
            schema_version: SCENE_VISUAL_ORDER_AUTHORING_MODEL_SCHEMA_VERSION.to_string(),
            scene_path,
            entries: Vec::new(),
            default_view_is_visual_order: true,
            debug_bucket_view_available: true,
            diagnostics: Vec::new(),
        }
    }

    pub fn runtime_gap_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !entry.runtime_supported)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiSceneUnifiedAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiSceneHitTestStatus {
    NotReady,
    NoPointer,
    Miss,
    HitAuiNode,
    HitSceneEntity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiSceneReorderStatus {
    NotRequested,
    Supported,
    RuntimeDeferred,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuiSceneUnifiedAuthoringReport {
    pub schema_version: String,
    pub status: AuiSceneUnifiedAuthoringStatus,
    pub scene_path: Option<String>,
    pub aui_document_count: usize,
    pub proxy_count: usize,
    pub selectable_proxy_count: usize,
    pub selected_target_kind: Option<String>,
    pub selected_document_path: Option<String>,
    pub selected_node_id: Option<String>,
    pub visual_order_entry_count: usize,
    pub selected_visual_order_key: Option<VisualOrderKey>,
    pub selected_visual_order_intent: Option<VisualOrderIntent>,
    pub visual_order_runtime_supported: bool,
    pub visual_order_runtime_support_reason: String,
    pub deferred_to_runtime_composition_gate: bool,
    pub reorder_supported: bool,
    pub last_reorder_status: AuiSceneReorderStatus,
    pub inspector_field_count: usize,
    pub hit_test_status: AuiSceneHitTestStatus,
    pub command_roundtrip_ok: bool,
    pub validation_ok: bool,
    pub glyph_present: bool,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl AuiSceneUnifiedAuthoringReport {
    pub fn empty(scene_path: Option<String>) -> Self {
        Self {
            schema_version: AUI_SCENE_UNIFIED_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status: AuiSceneUnifiedAuthoringStatus::Partial,
            scene_path,
            aui_document_count: 0,
            proxy_count: 0,
            selectable_proxy_count: 0,
            selected_target_kind: None,
            selected_document_path: None,
            selected_node_id: None,
            visual_order_entry_count: 0,
            selected_visual_order_key: None,
            selected_visual_order_intent: None,
            visual_order_runtime_supported: true,
            visual_order_runtime_support_reason:
                "before_world_screen_overlay_modal_authoring_supported".to_string(),
            deferred_to_runtime_composition_gate: false,
            reorder_supported: false,
            last_reorder_status: AuiSceneReorderStatus::NotRequested,
            inspector_field_count: 0,
            hit_test_status: AuiSceneHitTestStatus::NotReady,
            command_roundtrip_ok: false,
            validation_ok: true,
            glyph_present: false,
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    pub fn mark_runtime_deferred(&mut self, reason: impl Into<String>) {
        self.status = AuiSceneUnifiedAuthoringStatus::Partial;
        self.visual_order_runtime_supported = false;
        self.visual_order_runtime_support_reason = reason.into();
        self.deferred_to_runtime_composition_gate = true;
        self.last_reorder_status = AuiSceneReorderStatus::RuntimeDeferred;
        push_unique(
            &mut self.diagnostics,
            "deferred_to_runtime_composition_gate",
        );
        push_unique(
            &mut self.next_actions,
            "RuntimeRenderer Multi-stage UI Composition Pass",
        );
    }

    pub fn reject_single_aui_node_cross_world_reorder(&mut self) {
        self.status = AuiSceneUnifiedAuthoringStatus::Partial;
        self.reorder_supported = false;
        self.last_reorder_status = AuiSceneReorderStatus::Rejected;
        push_unique(
            &mut self.diagnostics,
            "extract_to_aui_layer_group_or_canvas",
        );
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}
