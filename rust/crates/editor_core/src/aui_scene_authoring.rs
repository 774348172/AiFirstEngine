use std::collections::HashMap;

use editor_ui_model::{
    AuiAuthoringVec2, AuiComputedAuthoringRect, AuiNodeAuthoringProxy, AuiSceneHitTestStatus,
    AuiSceneUnifiedAuthoringReport, AuiSceneUnifiedAuthoringStatus, AuiSceneViewProjection,
    AuiSourceRect, SceneVisualOrderAuthoringEntry, SceneVisualOrderAuthoringModel,
    SceneVisualOrderTargetKind, VisualOrderIntent, VisualOrderKey, WorkspaceSelectionTarget,
};
use engine_runtime::aui::{
    AuiCanvas, AuiCanvasMode, AuiCompositionStage, AuiComputedRect, AuiDocument, AuiLayoutEngine,
    AuiNode, AuiRect, AuiVec2,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AuiSceneAuthoringBuildOutput {
    pub proxies: Vec<AuiNodeAuthoringProxy>,
    pub visual_order: SceneVisualOrderAuthoringModel,
    pub report: AuiSceneUnifiedAuthoringReport,
}

pub struct AuiSceneAuthoringService;

impl AuiSceneAuthoringService {
    pub fn build_document_overlay(
        scene_path: Option<String>,
        document_path: impl Into<String>,
        document: &AuiDocument,
        projection: AuiSceneViewProjection,
        selected: Option<&WorkspaceSelectionTarget>,
    ) -> AuiSceneAuthoringBuildOutput {
        let document_path = document_path.into();
        let layout = AuiLayoutEngine::layout(document, 0);
        let nodes_by_id = document
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let canvases_by_id = document
            .canvases
            .iter()
            .map(|canvas| (canvas.canvas_id.as_str(), canvas))
            .collect::<HashMap<_, _>>();

        let mut proxies = Vec::new();
        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();

        for canvas in &document.canvases {
            let (key, runtime_supported, runtime_support_reason) =
                visual_order_key_for_canvas(canvas);
            if !runtime_supported {
                diagnostics.push(format!(
                    "aui_canvas_render_space_deferred:{}",
                    canvas.canvas_id
                ));
            }
            entries.push(SceneVisualOrderAuthoringEntry {
                entry_id: canvas_entry_id(&document_path, &canvas.canvas_id),
                display_name: canvas.canvas_id.clone(),
                target_kind: SceneVisualOrderTargetKind::AuiCanvas,
                target_ref: canvas.canvas_id.clone(),
                parent_entry_id: None,
                visual_order_key: key,
                visual_order_intent: VisualOrderIntent::none(),
                runtime_supported,
                runtime_support_reason,
                can_reorder: true,
                reorder_scope: "aui_canvas_visual_order".to_string(),
                diagnostics: Vec::new(),
            });
        }

        for computed in &layout.computed_nodes {
            let Some(node) = nodes_by_id.get(computed.node_id.as_str()) else {
                diagnostics.push(format!(
                    "aui_computed_node_missing_source:{}",
                    computed.node_id
                ));
                continue;
            };
            let Some(canvas) = canvases_by_id.get(computed.canvas_id.as_str()) else {
                diagnostics.push(format!(
                    "aui_computed_node_missing_canvas:{}:{}",
                    computed.canvas_id, computed.node_id
                ));
                continue;
            };
            let (mut key, runtime_supported, runtime_support_reason) =
                visual_order_key_for_canvas(canvas);
            key.local_order = computed.tree_order as i32;

            proxies.push(proxy_from_node(
                &document_path,
                &document.document_id,
                node,
                computed.rect,
            ));
            entries.push(SceneVisualOrderAuthoringEntry {
                entry_id: node_entry_id(&document_path, &node.node_id),
                display_name: node.name.clone(),
                target_kind: SceneVisualOrderTargetKind::AuiNode,
                target_ref: node.node_id.clone(),
                parent_entry_id: node.parent.as_ref().map_or_else(
                    || Some(canvas_entry_id(&document_path, &computed.canvas_id)),
                    |parent| Some(node_entry_id(&document_path, parent)),
                ),
                visual_order_key: key,
                visual_order_intent: VisualOrderIntent::none(),
                runtime_supported,
                runtime_support_reason,
                can_reorder: true,
                reorder_scope: "aui_sibling_order".to_string(),
                diagnostics: Vec::new(),
            });
        }

        entries.sort_by(|left, right| {
            (
                left.visual_order_key.render_space,
                left.visual_order_key.layer,
                left.visual_order_key.order,
                left.visual_order_key.local_order,
                left.entry_id.as_str(),
            )
                .cmp(&(
                    right.visual_order_key.render_space,
                    right.visual_order_key.layer,
                    right.visual_order_key.order,
                    right.visual_order_key.local_order,
                    right.entry_id.as_str(),
                ))
        });

        let selectable_proxy_count = proxies.iter().filter(|proxy| proxy.selectable).count();
        let mut visual_order = SceneVisualOrderAuthoringModel::empty(scene_path.clone());
        visual_order.entries = entries;
        visual_order.diagnostics = diagnostics.clone();

        let mut report = AuiSceneUnifiedAuthoringReport::empty(scene_path);
        report.status = if diagnostics.is_empty() {
            AuiSceneUnifiedAuthoringStatus::Passed
        } else {
            AuiSceneUnifiedAuthoringStatus::Partial
        };
        report.aui_document_count = 1;
        report.proxy_count = proxies.len();
        report.selectable_proxy_count = selectable_proxy_count;
        report.visual_order_entry_count = visual_order.entries.len();
        report.hit_test_status = match projection {
            AuiSceneViewProjection::Perspective | AuiSceneViewProjection::Orthographic2D => {
                AuiSceneHitTestStatus::NoPointer
            }
        };
        report.validation_ok = diagnostics.is_empty();
        report.diagnostics = diagnostics;

        if let Some(WorkspaceSelectionTarget::AuiNode {
            document_path: selected_document_path,
            document_id,
            node_id,
        }) = selected
        {
            report.selected_target_kind = Some("AuiNode".to_string());
            report.selected_document_path = Some(selected_document_path.clone());
            report.selected_node_id = Some(node_id.clone());
            if selected_document_path == &document_path && document_id == &document.document_id {
                report.selected_visual_order_key = visual_order
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.target_kind == SceneVisualOrderTargetKind::AuiNode
                            && entry.target_ref == *node_id
                    })
                    .map(|entry| entry.visual_order_key);
            }
        }

        if visual_order.runtime_gap_count() > 0 {
            report.mark_runtime_deferred(
                "document_contains_canvas_render_space_deferred_by_c_min_r1",
            );
        }

        AuiSceneAuthoringBuildOutput {
            proxies,
            visual_order,
            report,
        }
    }
}

fn visual_order_key_for_canvas(canvas: &AuiCanvas) -> (VisualOrderKey, bool, String) {
    match canvas.mode {
        AuiCanvasMode::ScreenOverlay => match canvas.composition_stage {
            AuiCompositionStage::BeforeWorld => (
                visual_order_key_for_stage(
                    canvas.composition_stage,
                    canvas.layer,
                    canvas.sorting_order,
                ),
                true,
                "before_world_runtime_pass_supported".to_string(),
            ),
            AuiCompositionStage::ScreenOverlay => (
                visual_order_key_for_stage(
                    canvas.composition_stage,
                    canvas.layer,
                    canvas.sorting_order,
                ),
                true,
                "screen_overlay_runtime_pass_supported".to_string(),
            ),
            AuiCompositionStage::Modal => (
                visual_order_key_for_stage(
                    canvas.composition_stage,
                    canvas.layer,
                    canvas.sorting_order,
                ),
                true,
                "modal_runtime_pass_supported".to_string(),
            ),
        },
        AuiCanvasMode::ScreenCamera | AuiCanvasMode::WorldSpace => (
            visual_order_key_for_stage(
                canvas.composition_stage,
                canvas.layer,
                canvas.sorting_order,
            ),
            false,
            "render_space_deferred_to_runtime_composition_gate".to_string(),
        ),
    }
}

fn visual_order_key_for_stage(
    stage: AuiCompositionStage,
    layer: i32,
    sorting_order: i32,
) -> VisualOrderKey {
    match stage {
        AuiCompositionStage::BeforeWorld => VisualOrderKey::before_world(layer, sorting_order, 0),
        AuiCompositionStage::ScreenOverlay => {
            VisualOrderKey::screen_overlay(layer, sorting_order, 0)
        }
        AuiCompositionStage::Modal => VisualOrderKey::modal(layer, sorting_order, 0),
    }
}

fn proxy_from_node(
    document_path: &str,
    document_id: &str,
    node: &AuiNode,
    rect: AuiComputedRect,
) -> AuiNodeAuthoringProxy {
    AuiNodeAuthoringProxy {
        document_path: document_path.to_string(),
        document_id: document_id.to_string(),
        node_id: node.node_id.clone(),
        parent_node_id: node.parent.clone(),
        name: node.name.clone(),
        kind: format!("{:?}", node.kind),
        source_rect: source_rect(node.rect),
        rect: computed_rect(rect),
        visible: node.visible,
        interactable: node.interactable,
        binding_count: node.binding_refs.len(),
        action_count: node.action_refs.len(),
        selectable: node.visible,
        diagnostics: Vec::new(),
    }
}

fn source_rect(rect: AuiRect) -> AuiSourceRect {
    AuiSourceRect {
        anchor_min: vec2(rect.anchor_min),
        anchor_max: vec2(rect.anchor_max),
        offset_min: vec2(rect.offset_min),
        offset_max: vec2(rect.offset_max),
        pivot: vec2(rect.pivot),
        size: vec2(rect.size),
    }
}

fn vec2(value: AuiVec2) -> AuiAuthoringVec2 {
    AuiAuthoringVec2 {
        x: value.x,
        y: value.y,
    }
}

fn computed_rect(rect: AuiComputedRect) -> AuiComputedAuthoringRect {
    AuiComputedAuthoringRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn canvas_entry_id(document_path: &str, canvas_id: &str) -> String {
    format!("aui-canvas:{document_path}:{canvas_id}")
}

fn node_entry_id(document_path: &str, node_id: &str) -> String {
    format!("aui-node:{document_path}:{node_id}")
}
