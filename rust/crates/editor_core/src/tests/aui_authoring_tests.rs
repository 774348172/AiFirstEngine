use super::*;
use engine_runtime::aui::{
    AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiNode, AuiNodeKind, AuiRect, AuiStyle,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("aif_editor_core_{name}_{stamp}"))
}

#[test]
fn aui_authoring_service_creates_saves_reopens_hud_document() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_style(AuiStyle::color("#101820"));
    let mut service = AuiAuthoringService::create_document("hud", 1280.0, 720.0, root);
    let add_text = service.add_node(
        "root",
        AuiNode::new(
            "score_text",
            AuiNodeKind::Text,
            AuiRect::fixed_position(16.0, 16.0, 220.0, 40.0),
        )
        .with_text("Score: 0"),
    );

    assert_eq!(add_text.status, AuiTransactionStatus::Committed);
    assert!(service.validate(None).ok);

    let dir = temp_project_path("save_reopen");
    let path = dir.join("Assets").join("UI").join("hud.aui.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    service.save(&path).unwrap();

    let reopened = AuiAuthoringService::open(&path).unwrap();
    assert_eq!(reopened.document().document_id, "hud");
    assert!(reopened
        .document()
        .nodes
        .iter()
        .any(|node| node.node_id == "score_text" && node.text.as_deref() == Some("Score: 0")));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn aui_authoring_service_edits_binding_by_schema_path() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["score_text"]);
    let score = AuiNode::new(
        "score_text",
        AuiNodeKind::Text,
        AuiRect::fixed_position(16.0, 16.0, 220.0, 40.0),
    )
    .with_parent("root")
    .with_text("Score: 0");
    let mut service = AuiAuthoringService::create_document("hud", 1280.0, 720.0, root);
    let add = service.add_node("root", score);
    assert_eq!(add.status, AuiTransactionStatus::Committed);

    let edit = service.set_node_field(
        "score_text",
        "bindingRefs",
        AuiNodeFieldValue::Binding(AuiBindingRef::new(
            "bind.score",
            AuiBindingTarget::TextText,
            "game.score_text",
            Some(AuiBindingValue::String("Score: 0".to_string())),
        )),
    );

    assert_eq!(edit.status, AuiTransactionStatus::Committed);
    let score_node = service
        .document()
        .nodes
        .iter()
        .find(|node| node.node_id == "score_text")
        .unwrap();
    assert_eq!(score_node.binding_refs.len(), 1);
    assert_eq!(score_node.binding_refs[0].path, "game.score_text");
    assert_eq!(service.report(None).transaction_count, 2);
}

#[test]
fn aui_authoring_service_upserts_binding_and_action_refs() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["button"]);
    let button = AuiNode::new(
        "button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(16.0, 16.0, 180.0, 48.0),
    )
    .with_parent("root");
    let mut service = AuiAuthoringService::create_document("hud", 1280.0, 720.0, root);
    assert_eq!(
        service.add_node("root", button).status,
        AuiTransactionStatus::Committed
    );

    assert_eq!(
        service
            .set_binding_path(
                "button",
                AuiBindingRef::new(
                    "bind.button",
                    AuiBindingTarget::PanelVisible,
                    "ui.button_visible",
                    Some(AuiBindingValue::Bool(true)),
                ),
            )
            .status,
        AuiTransactionStatus::Committed
    );
    assert_eq!(
        service
            .set_binding_path(
                "button",
                AuiBindingRef::new(
                    "bind.button",
                    AuiBindingTarget::PanelVisible,
                    "ui.button_enabled",
                    Some(AuiBindingValue::Bool(false)),
                ),
            )
            .status,
        AuiTransactionStatus::Committed
    );
    assert_eq!(
        service
            .set_action_ref(
                "button",
                engine_runtime::aui::AuiActionEvent::Click,
                "ui.pause"
            )
            .status,
        AuiTransactionStatus::Committed
    );
    assert_eq!(
        service
            .set_action_ref(
                "button",
                engine_runtime::aui::AuiActionEvent::Click,
                "ui.resume"
            )
            .status,
        AuiTransactionStatus::Committed
    );

    let button = service
        .document()
        .nodes
        .iter()
        .find(|node| node.node_id == "button")
        .unwrap();
    assert_eq!(button.binding_refs.len(), 1);
    assert_eq!(button.binding_refs[0].path, "ui.button_enabled");
    assert_eq!(button.action_refs.len(), 1);
    assert_eq!(button.action_refs[0].action_id, "ui.resume");
    assert_eq!(service.report(None).binding_count, 1);
    assert_eq!(service.report(None).action_count, 1);
}

#[test]
fn editor_session_executes_aui_authoring_command_chain() {
    let dir = temp_project_path("session_chain");
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateProject {
                path: dir.display().to_string(),
                name: "AUI Session".to_string(),
            }))
            .status,
        CommandStatus::Committed
    );

    let commands = vec![
        UiCommandPayload::CreateAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
            document_id: "hud".to_string(),
            width: 1280.0,
            height: 720.0,
        },
        UiCommandPayload::AddAuiNode {
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
        },
        UiCommandPayload::SetAuiNodeField {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            schema_path: "text".to_string(),
            value: serde_json::json!("Score: 0"),
        },
        UiCommandPayload::SetAuiBindingPath {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            target_field: "text.text".to_string(),
            binding_id: "bind.score".to_string(),
            binding_path: "game.score_text".to_string(),
            fallback: Some(serde_json::json!("Score: 0")),
        },
        UiCommandPayload::SetAuiActionRef {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            event: "click".to_string(),
            action_id: "ui.score.click".to_string(),
            payload: Some(serde_json::json!({"source": "test"})),
        },
        UiCommandPayload::ValidateAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        UiCommandPayload::SaveAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        UiCommandPayload::PreviewAuiOverlay {
            path: "AUI/hud.aui.json".to_string(),
        },
    ];

    for payload in commands {
        let result = session.execute_command(command_for_test(payload));
        assert_eq!(result.status, CommandStatus::Committed, "{result:?}");
    }

    let path = dir.join("AUI").join("hud.aui.json");
    let reopened = AuiAuthoringService::open(&path).unwrap();
    let score = reopened
        .document()
        .nodes
        .iter()
        .find(|node| node.node_id == "score_text")
        .unwrap();
    assert_eq!(score.text.as_deref(), Some("Score: 0"));
    assert_eq!(score.binding_refs[0].path, "game.score_text");
    assert_eq!(score.action_refs[0].action_id, "ui.score.click");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn editor_session_opens_legacy_aui_document_through_cooker_for_preview() {
    let dir = temp_project_path("legacy_preview");
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(command_for_test(UiCommandPayload::CreateProject {
                path: dir.display().to_string(),
                name: "Legacy AUI".to_string(),
            }))
            .status,
        CommandStatus::Committed
    );
    let aui_dir = dir.join("AUI");
    fs::create_dir_all(&aui_dir).unwrap();
    fs::write(
        aui_dir.join("hud.aui.json"),
        r##"{
  "schemaVersion": "aui-document.v1",
  "documentId": "hud-main",
  "root": {
    "nodeId": "hud-root",
    "nodeType": "canvas",
    "children": [
      {
        "nodeId": "score-label",
        "nodeType": "text",
        "text": "SCORE 000000",
        "anchor": "top-left"
      }
    ]
  }
}"##,
    )
    .unwrap();

    let open = session.execute_command(command_for_test(UiCommandPayload::OpenAuiDocument {
        path: "AUI/hud.aui.json".to_string(),
    }));
    assert_eq!(open.status, CommandStatus::Committed, "{open:?}");

    let preview = session.execute_command(command_for_test(UiCommandPayload::PreviewAuiOverlay {
        path: "AUI/hud.aui.json".to_string(),
    }));
    assert_eq!(preview.status, CommandStatus::Committed, "{preview:?}");
    assert!(!preview
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.aui_preview.aui_present.glyph_not_proven"));
    let _ = fs::remove_dir_all(dir);
}
