use super::*;
use editor_ui_model::{
    AuiSceneHitTestStatus, AuiSceneUnifiedAuthoringStatus, AuiSceneViewProjection,
    SceneVisualOrderRenderSpace, SceneVisualOrderTargetKind, WorkspaceSelectionTarget,
};
use engine_runtime::aui::{
    AuiCanvas, AuiCompositionStage, AuiDocument, AuiNode, AuiNodeKind, AuiRect,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("aif_editor_core_aui_scene_{name}_{stamp}"))
}

fn sample_aui_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["score_text"]);
    let score = AuiNode::new(
        "score_text",
        AuiNodeKind::Text,
        AuiRect::fixed_position(16.0, 16.0, 220.0, 40.0),
    )
    .with_parent("root")
    .with_text("Score: 0");
    AuiDocument::new(
        "hud",
        vec![AuiCanvas::screen_overlay(
            "hud_canvas",
            1280.0,
            720.0,
            "root",
        )],
        vec![root, score],
    )
}

#[test]
fn aui_scene_authoring_builds_proxies_from_runtime_layout() {
    let document = sample_aui_document();
    let output = AuiSceneAuthoringService::build_document_overlay(
        Some("Scenes/Main.scene.json".to_string()),
        "AUI/hud.aui.json",
        &document,
        AuiSceneViewProjection::Orthographic2D,
        None,
    );

    assert_eq!(output.report.status, AuiSceneUnifiedAuthoringStatus::Passed);
    assert_eq!(output.report.aui_document_count, 1);
    assert_eq!(output.report.proxy_count, 2);
    assert_eq!(output.report.selectable_proxy_count, 2);
    assert_eq!(
        output.report.hit_test_status,
        AuiSceneHitTestStatus::NoPointer
    );
    assert_eq!(output.visual_order.entries.len(), 3);
    assert!(output
        .visual_order
        .entries
        .iter()
        .any(|entry| entry.target_kind == SceneVisualOrderTargetKind::AuiCanvas));

    let score = output
        .proxies
        .iter()
        .find(|proxy| proxy.node_id == "score_text")
        .expect("score proxy");
    assert_eq!(score.document_path, "AUI/hud.aui.json");
    assert_eq!(score.document_id, "hud");
    assert_eq!(score.parent_node_id.as_deref(), Some("root"));
    assert_eq!(score.rect.x, 16.0);
    assert_eq!(score.rect.y, 16.0);
    assert_eq!(score.rect.width, 220.0);
    assert_eq!(score.rect.height, 40.0);
    assert!(score.selectable);
}

#[test]
fn aui_scene_authoring_reports_selected_aui_node_visual_order_key() {
    let document = sample_aui_document();
    let selected = WorkspaceSelectionTarget::AuiNode {
        document_path: "AUI/hud.aui.json".to_string(),
        document_id: "hud".to_string(),
        node_id: "score_text".to_string(),
    };

    let output = AuiSceneAuthoringService::build_document_overlay(
        Some("Scenes/Main.scene.json".to_string()),
        "AUI/hud.aui.json",
        &document,
        AuiSceneViewProjection::Perspective,
        Some(&selected),
    );

    assert_eq!(
        output.report.selected_target_kind.as_deref(),
        Some("AuiNode")
    );
    assert_eq!(
        output.report.selected_node_id.as_deref(),
        Some("score_text")
    );
    let key = output
        .report
        .selected_visual_order_key
        .expect("selected visual order key");
    assert_eq!(key.render_space, SceneVisualOrderRenderSpace::ScreenOverlay);
    assert!(output.report.visual_order_runtime_supported);
    assert!(!output.report.deferred_to_runtime_composition_gate);
}

#[test]
fn aui_scene_authoring_maps_before_world_canvas_to_supported_visual_order_key() {
    let mut document = sample_aui_document();
    document.canvases[0].composition_stage = AuiCompositionStage::BeforeWorld;
    document.canvases[0].layer = -1;
    document.canvases[0].sorting_order = 3;
    let selected = WorkspaceSelectionTarget::AuiNode {
        document_path: "AUI/hud.aui.json".to_string(),
        document_id: "hud".to_string(),
        node_id: "score_text".to_string(),
    };

    let output = AuiSceneAuthoringService::build_document_overlay(
        Some("Scenes/Main.scene.json".to_string()),
        "AUI/hud.aui.json",
        &document,
        AuiSceneViewProjection::Orthographic2D,
        Some(&selected),
    );

    let key = output
        .report
        .selected_visual_order_key
        .expect("selected visual order key");
    assert_eq!(key.render_space, SceneVisualOrderRenderSpace::BeforeWorld);
    assert_eq!(key.layer, -1);
    assert_eq!(key.order, 3);
    assert!(output.report.visual_order_runtime_supported);
    assert!(!output.report.deferred_to_runtime_composition_gate);
    assert_eq!(output.visual_order.runtime_gap_count(), 0);
    assert!(output
        .visual_order
        .entries
        .iter()
        .all(|entry| entry.runtime_supported));
}

#[test]
fn hierarchy_visual_order_entries_are_stable_for_aui_canvas_and_nodes() {
    let document = sample_aui_document();
    let output = AuiSceneAuthoringService::build_document_overlay(
        Some("Scenes/Main.scene.json".to_string()),
        "AUI/hud.aui.json",
        &document,
        AuiSceneViewProjection::Orthographic2D,
        None,
    );

    let entry_ids = output
        .visual_order
        .entries
        .iter()
        .map(|entry| entry.entry_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        entry_ids,
        vec![
            "aui-canvas:AUI/hud.aui.json:hud_canvas",
            "aui-node:AUI/hud.aui.json:root",
            "aui-node:AUI/hud.aui.json:score_text",
        ]
    );
    assert!(output.visual_order.default_view_is_visual_order);
    assert!(output.visual_order.debug_bucket_view_available);
    assert_eq!(output.visual_order.runtime_gap_count(), 0);
    assert!(output
        .visual_order
        .entries
        .iter()
        .all(|entry| entry.runtime_supported));
}

#[test]
fn aui_scene_inspector_command_roundtrip_updates_selected_node_field() {
    let dir = temp_project_path("select_aui_node");
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateProject {
                path: dir.display().to_string(),
                name: "AUI Scene".to_string(),
            }))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            }))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::AddAuiNode {
                path: "AUI/hud.aui.json".to_string(),
                parent_node_id: "root".to_string(),
                node_id: "score_text".to_string(),
                kind: "text".to_string(),
                name: "Score Text".to_string(),
                rect: serde_json::json!({
                    "x": 16.0,
                    "y": 16.0,
                    "width": 220.0,
                    "height": 40.0
                }),
            }))
            .status,
        CommandStatus::Committed
    );

    let select = session.execute_command(command_for_test(UiCommandPayload::SelectAuiNode {
        document_path: "AUI/hud.aui.json".to_string(),
        document_id: "hud".to_string(),
        node_id: "score_text".to_string(),
    }));
    assert_eq!(select.status, CommandStatus::Committed, "{select:?}");

    let model = session.build_ui_model();
    assert!(matches!(
        model.project_authoring_workspace.selection.primary,
        Some(editor_ui_model::WorkspaceSelectionTarget::AuiNode { ref node_id, .. })
            if node_id == "score_text"
    ));
    assert_eq!(
        model.hierarchy.authoring_view,
        editor_ui_model::HierarchyAuthoringView::VisualOrder
    );
    assert!(model
        .hierarchy
        .visual_order
        .as_ref()
        .is_some_and(|visual_order| visual_order
            .entries
            .iter()
            .any(|entry| entry.target_ref == "score_text")));
    assert!(model.inspector.title.contains("Score Text"));
    let inspector_field_count = model
        .inspector
        .sections
        .iter()
        .map(|section| section.fields.len())
        .sum::<usize>();
    assert!(inspector_field_count >= 10);
    assert!(model.inspector.sections.iter().any(|section| section
        .fields
        .iter()
        .any(|field| field.field_id == "aui.text")));

    let edit = session.execute_command(command_for_test(UiCommandPayload::SetAuiNodeField {
        path: "AUI/hud.aui.json".to_string(),
        node_id: "score_text".to_string(),
        schema_path: "text".to_string(),
        value: serde_json::json!("Score: 100"),
    }));
    assert_eq!(edit.status, CommandStatus::Committed, "{edit:?}");

    let reopened = AuiAuthoringService::open(&dir.join("AUI").join("hud.aui.json")).unwrap();
    let score = reopened
        .document()
        .nodes
        .iter()
        .find(|node| node.node_id == "score_text")
        .expect("score node");
    assert_eq!(score.text.as_deref(), Some("Score: 100"));

    let _ = fs::remove_dir_all(dir);
}
