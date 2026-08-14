use super::fixtures::*;
use super::*;

#[test]
fn editor_session_creates_empty_ui_model() {
    let session = EditorSession::new();
    let model = session.build_ui_model();
    assert_eq!(model.toolbar.runtime_state, RuntimeRunState::NoPackage);
    assert!(model.hierarchy.roots.is_empty());
    assert!(model.inspector.readonly);
}

#[test]
fn clear_console_is_snapshot_ready_transaction() {
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::ClearConsole));
    assert_eq!(result.status, CommandStatus::Committed);
    assert!(result
        .state_changes
        .iter()
        .any(|change| change.kind == "console.cleared"));
}

#[test]
fn editor_session_builds_schema_driven_property_tree_for_selected_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    let mut registry = ComponentSchemaRegistry::new();
    registry.register_object_schema(ObjectSchema {
        object_type: "engine.scene_entity".to_string(),
        display_name: "Scene Entity".to_string(),
        fields: vec![FieldSchema::new(
            "name",
            "Name",
            PropertyValueType::String,
            PropertyEditorKind::Text,
        )],
    });

    let result = session
        .build_property_tree_for_selected_entity(&registry)
        .expect("selected entity property tree");

    assert!(result
        .tree
        .nodes
        .iter()
        .any(|node| node.path.as_str() == "name"));
    assert!(result
        .tree
        .nodes
        .iter()
        .any(|node| node.metadata.component_type.as_deref() == Some("game.health")));
    assert!(result.report.invalid_schema_count >= 1);
}
