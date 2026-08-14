use super::fixtures::*;
use super::*;

#[test]
fn editor_session_prefab_stage_create_edit_save_and_instantiate() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let create_project =
        session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "PrefabGame".to_string(),
        }));
    assert_eq!(create_project.status, CommandStatus::Committed);
    let open_scene = session.execute_command(command_for_test(
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));
    assert_eq!(open_scene.status, CommandStatus::Committed);
    create_prefab_source_entity(&mut session);

    let create_prefab = session.execute_command(command_for_test(
        UiCommandPayload::CreatePrefabFromSelection {
            scene_path: Some("Scenes/Main.scene.json".to_string()),
            root_entity_id: "entity-player".to_string(),
            prefab_id: "prefab-player".to_string(),
            name: "Player Prefab".to_string(),
            replace_selection_with_instance: true,
        },
    ));
    assert_eq!(create_prefab.status, CommandStatus::Committed);
    assert!(root
        .join("Prefabs")
        .join("prefab-player.prefab.json")
        .exists());

    let open_prefab =
        session.execute_command(command_for_test(UiCommandPayload::OpenPrefabDocument {
            path: "Prefabs/prefab-player.prefab.json".to_string(),
        }));
    assert_eq!(open_prefab.status, CommandStatus::Committed);
    assert!(session.prefab_authoring.active_stage.is_some());

    let edit = session.execute_command(command_for_test(
        UiCommandPayload::SetPrefabStageEntityField {
            source_entity_id: "entity-player".to_string(),
            component_type: Some("game.health".to_string()),
            field_path: "hp".to_string(),
            value: serde_json::json!(7),
        },
    ));
    assert_eq!(edit.status, CommandStatus::Committed);
    assert!(
        session
            .prefab_authoring
            .active_stage
            .as_ref()
            .expect("active stage")
            .dirty
    );

    let save = session.execute_command(command_for_test(UiCommandPayload::SavePrefabDocument {
        path: "Prefabs/prefab-player.prefab.json".to_string(),
    }));
    assert_eq!(save.status, CommandStatus::Committed);
    let saved = PrefabWorkflowService::load_asset(&root, "Prefabs/prefab-player.prefab.json")
        .expect("saved prefab");
    assert_eq!(
        saved.entities[0].components[0].fields["hp"],
        serde_json::json!(7)
    );

    let instantiate = session.execute_command(command_for_test(
        UiCommandPayload::InstantiatePrefabInScene {
            prefab_id: "prefab-player".to_string(),
            parent_entity_id: None,
            local_position: Some(Vec3 {
                x: 2.0,
                y: 3.0,
                z: 0.0,
            }),
        },
    ));
    assert_eq!(instantiate.status, CommandStatus::Committed);
    assert!(session
        .editor_scene_document
        .as_ref()
        .expect("scene")
        .entities
        .iter()
        .any(|entity| entity
            .components
            .iter()
            .any(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)));
}

#[test]
fn editor_session_prefab_apply_and_revert_override_commands() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PrefabGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));
    create_prefab_source_entity(&mut session);
    session.execute_command(command_for_test(
        UiCommandPayload::CreatePrefabFromSelection {
            scene_path: None,
            root_entity_id: "entity-player".to_string(),
            prefab_id: "prefab-player".to_string(),
            name: "Player Prefab".to_string(),
            replace_selection_with_instance: true,
        },
    ));

    let override_edit =
        session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
            value: serde_json::json!(4),
        }));
    assert_eq!(override_edit.status, CommandStatus::Committed);
    let instance = PrefabInstance::from_scene_entity(
        session
            .editor_scene_document
            .as_ref()
            .expect("scene")
            .entity("entity-player")
            .expect("entity-player"),
    )
    .expect("prefab instance");
    assert_eq!(instance.overrides.len(), 1);

    let revert =
        session.execute_command(command_for_test(UiCommandPayload::RevertPrefabOverride {
            instance_entity_id: "entity-player".to_string(),
            target_source_entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
        }));
    assert_eq!(revert.status, CommandStatus::Committed);
    let instance = PrefabInstance::from_scene_entity(
        session
            .editor_scene_document
            .as_ref()
            .expect("scene")
            .entity("entity-player")
            .expect("entity-player"),
    )
    .expect("prefab instance");
    assert!(instance.overrides.is_empty());

    session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
        entity_id: "entity-player".to_string(),
        component_type: "game.health".to_string(),
        field_path: "hp".to_string(),
        value: serde_json::json!(5),
    }));
    let apply = session.execute_command(command_for_test(
        UiCommandPayload::ApplyPrefabOverrideToAsset {
            instance_entity_id: "entity-player".to_string(),
            target_source_entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
        },
    ));
    assert_eq!(apply.status, CommandStatus::Committed);
    let saved = PrefabWorkflowService::load_asset(&root, "Prefabs/prefab-player.prefab.json")
        .expect("saved prefab");
    assert_eq!(
        saved.entities[0].components[0].fields["hp"],
        serde_json::json!(5)
    );

    let validate = session.execute_command(command_for_test(
        UiCommandPayload::ValidatePrefabReferences { path: None },
    ));
    assert_eq!(validate.status, CommandStatus::Committed);
    assert_eq!(session.prefab_authoring_report().prefab_instances_count, 1);
}

fn create_prefab_source_entity(session: &mut EditorSession) {
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateSceneEntity {
        parent_id: None,
        name: "Player".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    let document = session
        .editor_scene_document
        .as_mut()
        .expect("scene document");
    let entity = document.entity_mut("entity-player").expect("entity-player");
    entity.components.push(EditorSceneComponent {
        component_type: "game.health".to_string(),
        fields: serde_json::json!({ "hp": 10, "maxHp": 10 }),
    });
}
