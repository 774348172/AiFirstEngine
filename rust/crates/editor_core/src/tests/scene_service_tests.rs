use super::fixtures::*;
use super::*;

#[test]
fn editor_session_open_scene_document_builds_hierarchy() {
    let scene_path = write_editor_scene_fixture();
    let mut session = EditorSession::new();

    let result = session.open_scene_document_for_test(&scene_path);

    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    assert_eq!(model.hierarchy.scene_id.as_deref(), Some("scene-main"));
    assert_eq!(model.hierarchy.roots.len(), 1);
    assert_eq!(model.hierarchy.roots[0].entity_id, "entity-player");
}

#[test]
fn scene_pick_viewport_select_entity_updates_workspace_selection() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(editor_ui_model::UiCommand {
        command_id: "select_scene_entity".to_string(),
        source: editor_ui_model::UiCommandSource::Viewport,
        request_id: "request-scene-pick".to_string(),
        payload: UiCommandPayload::SelectSceneEntity {
            entity_id: "entity-player".to_string(),
        },
    });

    assert_eq!(result.status, CommandStatus::Committed, "{result:?}");
    let model = session.build_ui_model();
    assert_eq!(
        model.hierarchy.selected_entity_id.as_deref(),
        Some("entity-player")
    );
}

#[test]
fn editor_session_scene_edit_create_entity_refreshes_hierarchy() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_scene_edit_for_test(SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Enemy".to_string(),
        mesh: None,
        components: Vec::new(),
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });

    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    assert!(model
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-enemy"));
}

#[test]
fn editor_session_scene_edit_set_transform_refreshes_viewport() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_scene_edit_for_test(SceneEditCommand::SetTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(EditorVec3 {
            x: 12.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    });

    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    let player = model
        .viewport
        .renderables
        .iter()
        .find(|renderable| renderable.entity_id == "entity-player")
        .expect("player renderable should exist");
    assert_eq!(player.local_position.x, 12.0);
}

#[test]
fn editor_session_scene_edit_failure_creates_console_error() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_scene_edit_for_test(SceneEditCommand::SetTransform {
        entity_id: "missing".to_string(),
        local_position: Some(EditorVec3::ZERO),
        local_rotation: None,
        local_scale: None,
    });

    assert_eq!(result.status, CommandStatus::Rejected);
    let model = session.build_ui_model();
    assert!(model.console.unread_error_count > 0);
    assert!(model
        .console
        .entries
        .iter()
        .any(|entry| entry.message.contains("rejected")));
}

#[test]
fn editor_session_save_scene_clears_dirty() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let edit = session.execute_scene_edit_for_test(SceneEditCommand::SetTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(EditorVec3 {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    });
    assert_eq!(edit.status, CommandStatus::Committed);
    assert!(
        session
            .editor_scene_document
            .as_ref()
            .unwrap()
            .dirty_state
            .dirty
    );

    let result = session.save_scene_document_for_test(Some(scene_path));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(
        !session
            .editor_scene_document
            .as_ref()
            .unwrap()
            .dirty_state
            .dirty
    );
}

#[test]
fn scene_edit_ui_model_shows_created_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_scene_edit_for_test(SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Enemy".to_string(),
        mesh: None,
        components: Vec::new(),
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });

    let model = session.build_ui_model();

    assert!(model
        .hierarchy
        .roots
        .iter()
        .any(|node| node.label == "Enemy"));
}

#[test]
fn editor_session_place_asset_into_scene_creates_selected_renderable_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: "model-enemy".to_string(),
        asset_type: "model".to_string(),
        asset_guid: Some("guid-model-enemy".to_string()),
        target_parent_id: None,
        local_position: Some(Vec3 {
            x: 4.0,
            y: 0.0,
            z: 0.0,
        }),
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.scene_dirty(), Some(true));
    let model = session.build_ui_model();
    assert_eq!(
        model.hierarchy.selected_entity_id.as_deref(),
        Some("entity-model-enemy")
    );
    assert!(model
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-model-enemy"));
    assert_eq!(model.viewport.renderable_count, 2);
    assert!(model
        .viewport
        .renderables
        .iter()
        .any(
            |renderable| renderable.mesh_ref.as_deref() == Some("model-enemy")
                && renderable.local_position.x == 4.0
        ));
    assert!(model.inspector.sections.iter().any(|section| {
            section.section_id == "mesh"
                && section.fields.iter().any(|field| {
                    matches!(&field.value, InspectorValue::AssetRef(value) if value.asset_id == "model-enemy")
                })
        }));
}

#[test]
fn editor_session_place_asset_into_scene_requires_open_scene_document() {
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: "model-enemy".to_string(),
        asset_type: "model".to_string(),
        asset_guid: None,
        target_parent_id: None,
        local_position: None,
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));

    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.scene_document.not_loaded"));
}

#[test]
fn editor_session_place_asset_into_scene_rejects_unsupported_type_without_dirtying_scene() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::PlaceAssetIntoScene {
        asset_id: "sound-laser".to_string(),
        asset_type: "audio".to_string(),
        asset_guid: None,
        target_parent_id: None,
        local_position: None,
        placement_mode: AssetPlacementMode::WorldOrigin,
    }));

    assert_eq!(result.status, CommandStatus::Rejected);
    assert_eq!(session.scene_dirty(), Some(false));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "asset_placement.asset_type_unsupported"));
    assert!(!session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-sound-laser"));
}

#[test]
fn scene_edit_ui_model_shows_selected_entity_transform() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_scene_edit_for_test(SceneEditCommand::SelectEntity {
        entity_id: "entity-player".to_string(),
    });

    let model = session.build_ui_model();

    assert_eq!(
        model.inspector.selected_entity_id.as_deref(),
        Some("entity-player")
    );
    assert!(model
        .inspector
        .sections
        .iter()
        .any(|section| section.section_id == "transform"));
}

#[test]
fn scene_edit_ui_model_shows_component_json_field() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_scene_edit_for_test(SceneEditCommand::SelectEntity {
        entity_id: "entity-player".to_string(),
    });

    let model = session.build_ui_model();

    assert!(model.inspector.sections.iter().any(|section| {
        section.section_id == "game.health"
            && section
                .fields
                .iter()
                .any(|field| matches!(field.value, InspectorValue::Json(_)))
    }));
}

#[test]
fn editor_session_execute_command_scene_opens_scene_document() {
    let scene_path = write_editor_scene_fixture();
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: scene_path.display().to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(
        session.build_ui_model().hierarchy.scene_id.as_deref(),
        Some("scene-main")
    );
}

#[test]
fn editor_session_execute_command_scene_selects_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(
        session
            .build_ui_model()
            .inspector
            .selected_entity_id
            .as_deref(),
        Some("entity-player")
    );
}

#[test]
fn editor_session_execute_command_scene_creates_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::CreateSceneEntity {
        parent_id: None,
        name: "Enemy".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-enemy"));
}

#[test]
fn editor_session_execute_command_scene_deletes_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::DeleteSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(session.build_ui_model().hierarchy.roots.is_empty());
}

#[test]
fn editor_session_execute_command_scene_sets_transform() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(player_view_x(&session), 5.0);
}

#[test]
fn editor_session_execute_command_scene_sets_component_field() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result =
        session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
            value: serde_json::json!(7),
        }));

    assert_eq!(result.status, CommandStatus::Committed);
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    let model = session.build_ui_model();
    let health = model
        .inspector
        .sections
        .iter()
        .find(|section| section.section_id == "game.health")
        .expect("health section");
    assert!(health.fields.iter().any(|field| {
        field.path == "components.game.health"
            && field.value
                == InspectorValue::Json(serde_json::json!({
                    "hp": 7,
                    "maxHp": 10
                }))
    }));
}

#[test]
fn editor_session_execute_command_scene_saves_document() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    make_scene_dirty(&mut session);

    let result = session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(
        !session
            .editor_scene_document
            .as_ref()
            .expect("scene document")
            .dirty_state
            .dirty
    );
}

#[test]
fn editor_session_execute_command_scene_undo_redo_scene_edit() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 9.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));

    let undo = session.execute_command(command_for_test(UiCommandPayload::UndoSceneEdit));
    assert_eq!(undo.status, CommandStatus::Committed);
    assert_eq!(player_view_x(&session), 0.0);

    let redo = session.execute_command(command_for_test(UiCommandPayload::RedoSceneEdit));
    assert_eq!(redo.status, CommandStatus::Committed);
    assert_eq!(player_view_x(&session), 9.0);
}

#[test]
fn editor_session_execute_command_scene_rejects_without_open_scene() {
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.scene_document.not_loaded"));
}

#[test]
fn scene_edit_test_api_matches_ui_command_path() {
    let scene_path = write_editor_scene_fixture();
    let mut ui_session = opened_editor_scene_session(&scene_path);
    let mut test_session = opened_editor_scene_session(&scene_path);

    let ui_result =
        ui_session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
            entity_id: "entity-player".to_string(),
            local_position: Some(Vec3 {
                x: 6.0,
                y: 0.0,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        }));
    let test_result = test_session.execute_scene_edit_for_test(SceneEditCommand::SetTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(EditorVec3 {
            x: 6.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    });

    assert_eq!(ui_result.status, CommandStatus::Committed);
    assert_eq!(test_result.status, CommandStatus::Committed);
    assert_eq!(player_view_x(&ui_session), player_view_x(&test_session));
}

#[test]
fn scene_save_test_api_matches_ui_command_path() {
    let scene_path = write_editor_scene_fixture();
    let mut ui_session = opened_editor_scene_session(&scene_path);
    let mut test_session = opened_editor_scene_session(&scene_path);
    make_scene_dirty(&mut ui_session);
    make_scene_dirty(&mut test_session);

    let ui_result =
        ui_session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
            path: None,
        }));
    let test_result = test_session.save_scene_document_for_test(None);

    assert_eq!(ui_result.status, CommandStatus::Committed);
    assert_eq!(test_result.status, CommandStatus::Committed);
    assert!(
        !ui_session
            .editor_scene_document
            .as_ref()
            .unwrap()
            .dirty_state
            .dirty
    );
    assert!(
        !test_session
            .editor_scene_document
            .as_ref()
            .unwrap()
            .dirty_state
            .dirty
    );
}

#[test]
fn inspector_scene_edit_invalid_component_field_reports_console_error() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result =
        session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "stats..hp".to_string(),
            value: serde_json::json!(7),
        }));

    assert_eq!(result.status, CommandStatus::Rejected);
    assert!(session.build_ui_model().console.unread_error_count > 0);
}

#[test]
fn ui_model_after_scene_open_has_scene_hierarchy() {
    let scene_path = write_editor_scene_fixture();
    let mut session = EditorSession::new();

    session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: scene_path.display().to_string(),
    }));

    let model = session.build_ui_model();
    assert_eq!(model.hierarchy.scene_id.as_deref(), Some("scene-main"));
    assert_eq!(model.viewport.renderable_count, 1);
}

#[test]
fn ui_model_after_scene_create_entity_refreshes_hierarchy() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    session.execute_command(command_for_test(UiCommandPayload::CreateSceneEntity {
        parent_id: None,
        name: "Enemy".to_string(),
    }));

    assert!(session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.label == "Enemy"));
}

#[test]
fn ui_model_after_scene_select_entity_refreshes_inspector() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    let model = session.build_ui_model();
    assert_eq!(
        model.inspector.selected_entity_id.as_deref(),
        Some("entity-player")
    );
    assert!(model
        .inspector
        .sections
        .iter()
        .any(|section| section.section_id == "transform"));
}

#[test]
fn editor_session_collider_overlay_marks_selected_entity() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let result =
        session.execute_command(command_for_test(UiCommandPayload::SetSceneComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "engine.collider2d".to_string(),
            field_path: "halfExtents.x".to_string(),
            value: serde_json::json!(0.75),
        }));
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    let overlay = session.build_ui_model().viewport.collider_overlay;

    assert_eq!(result.status, CommandStatus::Rejected);
    assert_eq!(overlay.collider_count, 0);

    session.execute_scene_edit_for_test(SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Collider".to_string(),
        mesh: None,
        components: vec![EditorSceneComponent {
            component_type: "engine.collider2d".to_string(),
            fields: serde_json::json!({
                "shape": "aabb",
                "halfExtents": { "x": 0.5, "y": 0.5 },
                "offset": { "x": 0.0, "y": 0.0 },
                "enabled": true,
                "isSensor": false
            }),
        }],
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-collider".to_string(),
    }));
    let overlay = session.build_ui_model().viewport.collider_overlay;

    assert_eq!(overlay.collider_count, 1);
    assert_eq!(overlay.draw_item_count, 1);
    assert_eq!(
        overlay.selected_entity_id.as_deref(),
        Some("entity-collider")
    );
    assert!(overlay.draw_items.iter().any(|item| {
        item.entity_id == "entity-collider" && item.selected && item.center.x == 0.0
    }));
}

#[test]
fn ui_model_after_scene_set_transform_refreshes_viewport() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 8.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));

    assert_eq!(player_view_x(&session), 8.0);
}

#[test]
fn editor_session_renames_scene_entity_from_ui_command() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::RenameSceneEntity {
        entity_id: "entity-player".to_string(),
        name: "Hero".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.build_ui_model().hierarchy.roots[0].label, "Hero");
}

#[test]
fn ui_model_after_scene_save_document_clears_dirty() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    make_scene_dirty(&mut session);

    session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));

    assert!(
        !session
            .editor_scene_document
            .as_ref()
            .unwrap()
            .dirty_state
            .dirty
    );
}

#[test]
fn ui_model_after_scene_failed_edit_adds_console_error() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "missing".to_string(),
        local_position: Some(Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));

    assert!(session.build_ui_model().console.unread_error_count > 0);
}
