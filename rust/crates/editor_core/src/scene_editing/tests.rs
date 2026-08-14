use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn editor_scene_document_loads_minimal_scene_file() {
    let path = write_scene_fixture(true);
    let document = EditorSceneDocument::load_from_path(&path).unwrap();
    assert_eq!(document.scene_id, "scene-main");
    assert_eq!(document.entities.len(), 1);
    assert_eq!(document.entities[0].entity_id, "entity-player");
    assert!(!document.dirty_state.dirty);
}

#[test]
fn editor_scene_document_serializes_stably() {
    let document = scene_document();
    let json = document.to_stable_json().unwrap();
    assert!(json.contains("\"schemaVersion\": \"editor-scene-document.v1\""));
    assert!(json.contains("\"localPosition\""));
    assert!(!json.contains("dirty_state"));
}

#[test]
fn editor_scene_document_requires_transform_for_each_entity() {
    let path = write_scene_fixture(false);
    let diagnostics = EditorSceneDocument::load_from_path(&path).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "scene.entity.transform_required"));
}

#[test]
fn editor_scene_document_tracks_dirty_revision() {
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    assert!(document.dirty_state.dirty);
    assert_eq!(document.revision, 1);
    assert_eq!(
        document.dirty_state.last_transaction_id.as_deref(),
        Some("tx-1")
    );
    document.clear_dirty();
    assert!(!document.dirty_state.dirty);
}

#[test]
fn scene_selection_selects_single_entity() {
    let document = scene_document();
    let mut selection = SceneSelection::default();
    assert!(selection.select_single(&document, "entity-player"));
    assert_eq!(
        selection.primary_entity_id.as_deref(),
        Some("entity-player")
    );
}

#[test]
fn scene_selection_clears_missing_entity() {
    let document = scene_document();
    let mut selection = SceneSelection::default();
    selection.select_single(&document, "entity-player");
    assert!(!selection.select_single(&document, "missing"));
    assert!(selection.primary_entity_id.is_none());
}

#[test]
fn scene_selection_does_not_mark_scene_dirty() {
    let document = scene_document();
    let mut selection = SceneSelection::default();
    selection.select_single(&document, "entity-player");
    assert!(!document.dirty_state.dirty);
}

#[test]
fn scene_edit_command_serializes_for_ai_patch() {
    let command = SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "Enemy".to_string(),
        mesh: None,
        components: Vec::new(),
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("\"commandType\":\"createEntity\""));
    assert!(json.contains("\"localTransform\""));
}

#[test]
fn scene_edit_request_records_source() {
    let request = request(SceneEditCommand::SelectEntity {
        entity_id: "entity-player".to_string(),
    });
    assert_eq!(request.source, SceneEditRequestSource::Test);
    assert_eq!(request.target_scene_id, "scene-main");
}

#[test]
fn scene_edit_command_set_transform_only_uses_local_transform() {
    let command = SceneEditCommand::SetTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(EditorVec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
        local_rotation: None,
        local_scale: None,
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("localPosition"));
    assert!(!json.contains("worldPosition"));
}

#[test]
fn scene_edit_command_save_scene_keeps_path_optional() {
    let command = SceneEditCommand::SaveScene {
        scene_id: "scene-main".to_string(),
        path: None,
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("\"path\":null"));
}

#[test]
fn scene_edit_transaction_create_entity_marks_dirty() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::CreateEntity {
            parent_id: None,
            name: "Enemy".to_string(),
            mesh: None,
            components: Vec::new(),
            local_transform: EditorTransform::identity(),
            sibling_order: None,
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert!(report.dirty_after);
    assert!(document.has_entity("entity-enemy"));
}

#[test]
fn scene_edit_transaction_select_entity_does_not_mark_dirty() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SelectEntity {
            entity_id: "entity-player".to_string(),
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert!(!document.dirty_state.dirty);
}

#[test]
fn scene_edit_transaction_set_transform_records_write_set() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SetTransform {
            entity_id: "entity-player".to_string(),
            local_position: Some(EditorVec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert!(report
        .write_set
        .iter()
        .any(|path| path.ends_with("transform.localPosition")));
}

#[test]
fn scene_edit_transaction_rejects_missing_entity() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SetTransform {
            entity_id: "missing".to_string(),
            local_position: Some(EditorVec3::ZERO),
            local_rotation: None,
            local_scale: None,
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Rejected);
    assert!(!document.dirty_state.dirty);
}

#[test]
fn scene_edit_transaction_rejects_cyclic_reparent() {
    let mut document = scene_document_with_child();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::ReparentEntity {
            entity_id: "entity-player".to_string(),
            new_parent_id: Some("entity-child".to_string()),
            sibling_order: None,
            keep_world_transform: false,
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Rejected);
}

#[test]
fn scene_edit_transaction_delete_entity_removes_child_subtree() {
    let mut document = scene_document_with_child();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::DeleteEntity {
            entity_id: "entity-player".to_string(),
            delete_children: true,
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert!(document.entities.is_empty());
}

#[test]
fn scene_edit_transaction_rename_entity_marks_dirty() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::RenameEntity {
            entity_id: "entity-player".to_string(),
            name: "Hero".to_string(),
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert!(report.dirty_after);
    assert_eq!(document.entities[0].name, "Hero");
    assert!(report
        .write_set
        .iter()
        .any(|path| path.ends_with("entity-player.name")));
}

#[test]
fn scene_edit_transaction_rejects_empty_entity_name() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::RenameEntity {
            entity_id: "entity-player".to_string(),
            name: "   ".to_string(),
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Rejected);
    assert_eq!(document.entities[0].name, "Player");
}

#[test]
fn scene_edit_transaction_set_component_field_updates_json_field() {
    let mut document = scene_document();
    document.entities[0].components.push(EditorSceneComponent {
        component_type: "game.health".to_string(),
        fields: serde_json::json!({ "hp": 10 }),
    });
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SetComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "game.health".to_string(),
            field_path: "hp".to_string(),
            value: serde_json::json!(20),
        }),
    );
    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert_eq!(
        document.entities[0].components[0].fields["hp"],
        serde_json::json!(20)
    );
}

#[test]
fn scene_edit_transaction_set_collider2d_nested_field_updates_json_field() {
    let mut document = scene_document();
    document.entities[0].components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "aabb",
            "halfExtents": { "x": 0.5, "y": 0.5 },
            "offset": { "x": 0.0, "y": 0.0 },
            "enabled": true,
            "isSensor": false
        }),
    });
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();

    let report = SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SetComponentField {
            entity_id: "entity-player".to_string(),
            component_type: "engine.collider2d".to_string(),
            field_path: "halfExtents.x".to_string(),
            value: serde_json::json!(1.25),
        }),
    );

    assert_eq!(report.status, SceneEditTransactionStatus::Committed);
    assert_eq!(
        document.entities[0].components[0].fields["halfExtents"]["x"],
        serde_json::json!(1.25)
    );
    let (world, _) = PreviewWorldSync::full_rebuild(&document).unwrap();
    let collider = world
        .collider2d(&EntityId::from("entity-player"))
        .expect("collider should sync");
    assert_eq!(
        collider.shape,
        Shape2D::Aabb {
            half_extents: Vec2 { x: 1.25, y: 0.5 }
        }
    );
}

#[test]
fn collider_debug_draw_list_builds_selected_aabb_and_circle() {
    let mut document = scene_document();
    document.entities[0].components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "aabb",
            "halfExtents": { "x": 0.5, "y": 0.75 },
            "offset": { "x": 1.0, "y": -1.0 },
            "layer": 2,
            "mask": 4,
            "enabled": true,
            "isSensor": true
        }),
    });
    let mut circle = EditorSceneEntity::new("entity-circle", "Circle");
    circle.components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "circle",
            "radius": 0.25
        }),
    });
    document.entities.push(circle);
    let mut selection = SceneSelection::default();
    selection.select_single(&document, "entity-player");

    let list = ColliderDebugDrawList::build(&document, &selection);

    assert_eq!(list.collider_count, 2);
    assert_eq!(list.draw_item_count, 2);
    assert_eq!(list.selected_entity_id.as_deref(), Some("entity-player"));
    assert!(list.draw_items.iter().any(|item| {
        item.entity_id == "entity-player"
            && item.selected
            && item.sensor
            && item.layer == 2
            && item.mask == 4
            && item.center.x == 1.0
            && item.center.y == -1.0
    }));
    assert!(list
        .draw_items
        .iter()
        .any(|item| matches!(item.shape, ColliderDebugShape::Circle { radius } if radius == 0.25)));
}

#[test]
fn collider_debug_draw_list_reports_missing_transform_and_invalid_size() {
    let mut document = scene_document();
    document.entities[0].components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "circle",
            "radius": -1.0
        }),
    });
    let mut no_transform = EditorSceneEntity::new("entity-no-transform", "No Transform");
    no_transform.transform = None;
    no_transform.components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "aabb",
            "halfExtents": { "x": 0.5, "y": 0.5 }
        }),
    });
    document.entities.push(no_transform);
    let selection = SceneSelection::default();

    let list = ColliderDebugDrawList::build(&document, &selection);

    assert_eq!(list.collider_count, 2);
    assert_eq!(list.draw_item_count, 0);
    assert_eq!(list.invalid_collider_count, 1);
    assert_eq!(list.missing_transform_count, 1);
    assert!(list
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field_path == "shape"));
    assert!(list
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field_path == "transform"));
}

#[test]
fn collider2d_authoring_scene_save_reload_and_runtime_pair_report() {
    let root = temp_root("collider-authoring-runtime");
    let path = root.join("scenes").join("main.scene.json");
    let mut document = EditorSceneDocument::new("scene-main", "Main");
    let mut left = EditorSceneEntity::new("entity-left", "Left");
    left.components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "aabb",
            "halfExtents": { "x": 0.5, "y": 0.5 }
        }),
    });
    let mut right = EditorSceneEntity::new("entity-right", "Right");
    right.transform.as_mut().unwrap().local_position.x = 0.25;
    right.components.push(EditorSceneComponent {
        component_type: "engine.collider2d".to_string(),
        fields: serde_json::json!({
            "shape": "circle",
            "radius": 0.5
        }),
    });
    document.entities.push(left);
    document.entities.push(right);
    document.mark_dirty("tx-collider");

    let save = SceneSavePipeline::save(&mut document, &root, Some(&path));
    let reloaded = EditorSceneDocument::load_from_path(&path).unwrap();
    let (world, _) = PreviewWorldSync::full_rebuild(&reloaded).unwrap();
    let mut physics_world = engine_runtime::physics2d::Physics2DWorld::new();
    let sync_report =
        engine_runtime::physics2d::Physics2DBridge::sync_from_world(&world, &mut physics_world);
    let pair_report = physics_world.build_collision_pairs();

    assert_eq!(save.status, SceneSaveStatus::Saved);
    assert_eq!(sync_report.synced_colliders, 2);
    assert_eq!(pair_report.pairs.len(), 1);
}

#[test]
fn scene_undo_restores_created_entity() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    create_enemy(&mut document, &mut selection, &mut undo);
    assert!(document.has_entity("entity-enemy"));
    undo.undo(&mut document);
    assert!(!document.has_entity("entity-enemy"));
}

#[test]
fn scene_redo_restores_created_entity() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    create_enemy(&mut document, &mut selection, &mut undo);
    undo.undo(&mut document);
    undo.redo(&mut document);
    assert!(document.has_entity("entity-enemy"));
}

#[test]
fn scene_undo_restores_transform() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SetTransform {
            entity_id: "entity-player".to_string(),
            local_position: Some(EditorVec3 {
                x: 5.0,
                y: 0.0,
                z: 0.0,
            }),
            local_rotation: None,
            local_scale: None,
        }),
    );
    undo.undo(&mut document);
    assert_eq!(
        document.entities[0].transform.unwrap().local_position,
        EditorVec3::ZERO
    );
}

#[test]
fn scene_undo_does_not_record_selection_only() {
    let mut document = scene_document();
    let mut selection = SceneSelection::default();
    let mut undo = SceneUndoStack::default();
    SceneEditTransaction::apply(
        "tx-1",
        &mut document,
        &mut selection,
        &mut undo,
        request(SceneEditCommand::SelectEntity {
            entity_id: "entity-player".to_string(),
        }),
    );
    assert!(undo.undo(&mut document).is_none());
}

#[test]
fn preview_world_sync_full_rebuild_creates_world_entities() {
    let document = scene_document();
    let (world, report) = PreviewWorldSync::full_rebuild(&document).unwrap();
    assert_eq!(world.entity_count(), 1);
    assert_eq!(report.entity_count, 1);
    assert!(world.entity(&EntityId::from("entity-player")).is_some());
}

#[test]
fn preview_world_sync_reports_entity_and_component_count() {
    let mut document = scene_document();
    document.entities[0].mesh = Some(mesh());
    document.entities[0].components.push(EditorSceneComponent {
        component_type: "game.health".to_string(),
        fields: serde_json::json!({ "hp": 10 }),
    });
    let (_, report) = PreviewWorldSync::full_rebuild(&document).unwrap();
    assert_eq!(report.entity_count, 1);
    assert_eq!(report.component_count, 3);
}

#[test]
fn preview_world_sync_does_not_clear_dirty() {
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    PreviewWorldSync::full_rebuild(&document).unwrap();
    assert!(document.dirty_state.dirty);
}

#[test]
fn preview_world_sync_rebuild_after_transform_change_updates_world() {
    let mut document = scene_document();
    document.entities[0]
        .transform
        .as_mut()
        .unwrap()
        .local_position
        .x = 8.0;
    let (world, _) = PreviewWorldSync::full_rebuild(&document).unwrap();
    assert_eq!(
        world
            .transform(&EntityId::from("entity-player"))
            .unwrap()
            .local_position
            .x,
        8.0
    );
}

#[test]
fn scene_save_pipeline_writes_scene_file() {
    let root = temp_root("scene-save");
    let path = root.join("scenes").join("main.scene.json");
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    let report = SceneSavePipeline::save(&mut document, &root, Some(&path));
    assert_eq!(report.status, SceneSaveStatus::Saved);
    assert!(path.exists());
}

#[test]
fn scene_save_pipeline_rejects_path_outside_project() {
    let root = temp_root("scene-save-outside");
    let outside = temp_root("outside").join("main.scene.json");
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    let report = SceneSavePipeline::save(&mut document, &root, Some(&outside));
    assert_eq!(report.status, SceneSaveStatus::Failed);
    assert!(document.dirty_state.dirty);
}

#[test]
fn scene_save_pipeline_rejects_runtime_package_output_path() {
    let root = temp_root("scene-save-runtime-package");
    let path = root.join("runtime-package").join("scene.json");
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    let report = SceneSavePipeline::save(&mut document, &root, Some(&path));
    assert_eq!(report.status, SceneSaveStatus::Failed);
}

#[test]
fn scene_save_pipeline_keeps_dirty_on_failure() {
    let root = temp_root("scene-save-failure");
    let path = root.join("runtime_package").join("scene.json");
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    let report = SceneSavePipeline::save(&mut document, &root, Some(&path));
    assert_eq!(report.status, SceneSaveStatus::Failed);
    assert!(document.dirty_state.dirty);
}

#[test]
fn scene_save_pipeline_clears_dirty_on_success() {
    let root = temp_root("scene-save-clean");
    let path = root.join("scenes").join("main.scene.json");
    let mut document = scene_document();
    document.mark_dirty("tx-1");
    let report = SceneSavePipeline::save(&mut document, &root, Some(&path));
    assert_eq!(report.status, SceneSaveStatus::Saved);
    assert!(!document.dirty_state.dirty);
}

#[test]
fn scene_save_pipeline_reload_matches_saved_document() {
    let root = temp_root("scene-save-reload");
    let path = root.join("scenes").join("main.scene.json");
    let mut document = scene_document();
    SceneSavePipeline::save(&mut document, &root, Some(&path));
    let reloaded = EditorSceneDocument::load_from_path(&path).unwrap();
    assert_eq!(
        reloaded.entities[0].entity_id,
        document.entities[0].entity_id
    );
}

fn create_enemy(
    document: &mut EditorSceneDocument,
    selection: &mut SceneSelection,
    undo: &mut SceneUndoStack,
) {
    SceneEditTransaction::apply(
        "tx-1",
        document,
        selection,
        undo,
        request(SceneEditCommand::CreateEntity {
            parent_id: None,
            name: "Enemy".to_string(),
            mesh: None,
            components: Vec::new(),
            local_transform: EditorTransform::identity(),
            sibling_order: None,
        }),
    );
}

fn request(command: SceneEditCommand) -> SceneEditRequest {
    SceneEditRequest {
        request_id: "request-test".to_string(),
        source: SceneEditRequestSource::Test,
        target_scene_id: "scene-main".to_string(),
        command,
    }
}

fn scene_document() -> EditorSceneDocument {
    let mut document = EditorSceneDocument::new("scene-main", "Main");
    let mut entity = EditorSceneEntity::new("entity-player", "Player");
    entity.kind = "player".to_string();
    document.entities.push(entity);
    document
}

fn scene_document_with_child() -> EditorSceneDocument {
    let mut document = scene_document();
    let mut child = EditorSceneEntity::new("entity-child", "Child");
    child.parent_id = Some("entity-player".to_string());
    document.entities.push(child);
    document
}

fn mesh() -> EditorMesh {
    EditorMesh {
        primitive: Some("model".to_string()),
        asset_ref: Some(EditorAssetRef {
            asset_id: "model-player".to_string(),
            asset_type_id: "model".to_string(),
            guid: None,
            sub_asset_id: None,
        }),
        material_ref: None,
        visible: true,
        layer: "default".to_string(),
    }
}

fn write_scene_fixture(include_transform: bool) -> PathBuf {
    let root = temp_root("editor-scene-document");
    fs::create_dir_all(root.join("scenes")).unwrap();
    let transform = if include_transform {
        r#""transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },"#
    } else {
        ""
    };
    let path = root.join("scenes").join("main.scene.json");
    fs::write(
        &path,
        format!(
            r##"{{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    {}
    "components": []
  }}]
}}"##,
            transform
        ),
    )
    .unwrap();
    path
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("{name}-{stamp}"));
    fs::create_dir_all(&root).unwrap();
    root
}
