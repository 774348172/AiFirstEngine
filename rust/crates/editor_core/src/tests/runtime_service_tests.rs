use super::fixtures::*;
use super::*;
use std::sync::{Mutex, OnceLock};

static RUNTIME_INSTANCE_START_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn tick_without_runtime_package_returns_structured_diagnostic() {
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::TickOneFrame));
    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.runtime_package.not_loaded"));
}

#[test]
fn open_runtime_package_builds_hierarchy_model() {
    let package_dir = write_runtime_package_fixture();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
        path: package_dir.display().to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    assert_eq!(model.hierarchy.roots.len(), 1);
    assert_eq!(model.hierarchy.roots[0].children.len(), 1);
    assert_eq!(
        model.hierarchy.source_domain,
        editor_ui_model::HierarchySourceDomain::OpenedRuntimePackage
    );
}

#[test]
fn select_entity_updates_readonly_inspector() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);
    let result = session.execute_command(command_for_test(UiCommandPayload::SelectEntity {
        entity_id: "entity-gun".to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    assert!(result
        .state_changes
        .iter()
        .any(|change| change.path == "selection.selected_entity_id"));
    assert_eq!(
        session.selected_entity_source,
        Some(EntitySelectionSource::OpenedRuntimePackage)
    );
    let model = session.build_ui_model();
    assert_eq!(
        model.inspector.selected_entity_id.as_deref(),
        Some("entity-gun")
    );
    assert!(model.inspector.readonly);
    assert_eq!(
        model.inspector.persistence,
        editor_ui_model::InspectorPersistence::ReadOnlyRuntimePackage
    );
    assert!(model
        .inspector
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .all(|field| field.readonly));
}

#[test]
fn runtime_pick_blocked_by_aui_keeps_selection_empty() {
    let package_dir = write_runtime_package_fixture();
    let mut session = EditorSession::new();
    attach_active_runtime_instance(&mut session, &package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
        x: 440.0,
        y: 300.0,
        viewport_width: Some(800.0),
        viewport_height: Some(600.0),
        aui_consumed: true,
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "editor.runtime_selection.pick"
            && diagnostic.message.contains("blocked_by_aui")
    }));
    assert_eq!(session.selected_entity_id, None);
    assert_eq!(session.selected_entity_source, None);
}

#[test]
fn runtime_pick_hits_live_world_and_temporary_inspector_preempts_scene_selection() {
    let package_dir = write_runtime_package_fixture();
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let scene_select =
        session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
            entity_id: "entity-player".to_string(),
        }));
    assert_eq!(scene_select.status, CommandStatus::Committed);
    attach_active_runtime_instance(&mut session, &package_dir);
    let hierarchy = session.build_ui_model().hierarchy;
    assert_eq!(
        hierarchy.source_domain,
        editor_ui_model::HierarchySourceDomain::ActiveGameViewRuntime
    );
    assert_eq!(hierarchy.roots[0].entity_id, "entity-player");

    let result = session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
        x: 440.0,
        y: 300.0,
        viewport_width: Some(800.0),
        viewport_height: Some(600.0),
        aui_consumed: false,
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.selected_entity_id.as_deref(), Some("entity-gun"));
    assert_eq!(
        session.selected_entity_source,
        Some(EntitySelectionSource::ActiveGameViewRuntime)
    );
    assert!(result.state_changes.iter().any(|change| {
        change.path == "selection.selected_entity_source"
            && change.after_summary.as_deref() == Some("active_game_view_runtime")
    }));

    let inspector = session.build_ui_model().inspector;
    assert_eq!(inspector.selected_entity_id.as_deref(), Some("entity-gun"));
    assert_eq!(inspector.title, "Runtime / Temporary: Gun");
    assert!(!inspector.readonly);
    assert_eq!(
        inspector.persistence,
        editor_ui_model::InspectorPersistence::TemporaryPlaySession
    );
    assert!(inspector
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .any(|field| field.field_id == "transform.localPosition" && field.editable));
    assert!(inspector.sections.iter().any(|section| {
        section.section_id == "metadata"
            && section.fields.iter().any(|field| {
                field.field_id == "metadata.selectionSource"
                    && matches!(
                        &field.value,
                        InspectorValue::String(value) if value == "active_game_view_runtime"
                    )
            })
    }));
}

#[test]
fn runtime_pick_miss_reports_without_replacing_existing_runtime_selection() {
    let package_dir = write_runtime_package_fixture();
    let mut session = EditorSession::new();
    attach_active_runtime_instance(&mut session, &package_dir);
    let hit = session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
        x: 440.0,
        y: 300.0,
        viewport_width: Some(800.0),
        viewport_height: Some(600.0),
        aui_consumed: false,
    }));
    assert_eq!(hit.status, CommandStatus::Committed);
    assert_eq!(session.selected_entity_id.as_deref(), Some("entity-gun"));

    let miss = session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
        x: 760.0,
        y: 540.0,
        viewport_width: Some(800.0),
        viewport_height: Some(600.0),
        aui_consumed: false,
    }));

    assert_eq!(miss.status, CommandStatus::Committed);
    assert!(miss.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "editor.runtime_selection.pick" && diagnostic.message.contains("miss")
    }));
    assert_eq!(session.selected_entity_id.as_deref(), Some("entity-gun"));
    assert_eq!(
        session.selected_entity_source,
        Some(EntitySelectionSource::ActiveGameViewRuntime)
    );
}

#[test]
fn runtime_hierarchy_selects_active_runtime_entity() {
    let package_dir = write_runtime_package_fixture();
    let mut session = EditorSession::new();
    attach_active_runtime_instance(&mut session, &package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::SelectRuntimeEntity {
        entity_id: "entity-gun".to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.selected_entity_id.as_deref(), Some("entity-gun"));
    assert_eq!(
        session.selected_entity_source,
        Some(EntitySelectionSource::ActiveGameViewRuntime)
    );
    assert!(result.state_changes.iter().any(|change| {
        change.path == "selection.selected_entity_source"
            && change.after_summary.as_deref() == Some("active_game_view_runtime")
    }));
}

#[test]
fn runtime_temporary_edit_updates_live_world_survives_step_and_discards_on_stop() {
    let package_dir = write_runtime_package_fixture();
    let mut session = EditorSession::new();
    attach_active_runtime_instance(&mut session, &package_dir);
    let select = session.execute_command(command_for_test(UiCommandPayload::SelectRuntimeEntity {
        entity_id: "entity-gun".to_string(),
    }));
    assert_eq!(select.status, CommandStatus::Committed);

    let edit = session.execute_command(command_for_test(
        UiCommandPayload::SetRuntimeComponentFieldTemporary {
            entity_id: "entity-gun".to_string(),
            component_type: "Transform".to_string(),
            field_path: "local_position.x".to_string(),
            value: serde_json::json!(9.0),
        },
    ));

    assert_eq!(edit.status, CommandStatus::Committed);
    assert!(edit
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "editor.runtime_temporary_edit.applied" }));
    let entity_id = engine_runtime::ids::EntityId::new("entity-gun");
    let instance = session
        .editor_runtime_play_instance
        .as_ref()
        .expect("active runtime instance");
    assert_eq!(
        instance
            .runtime_world()
            .transform(&entity_id)
            .expect("gun transform")
            .local_position
            .x,
        9.0
    );
    assert_eq!(instance.temporary_edit_summary().edited_field_count, 1);

    let pause = session.execute_command(command_for_test(UiCommandPayload::Pause));
    assert_eq!(pause.status, CommandStatus::Committed);
    let step = session.execute_command(command_for_test(UiCommandPayload::StepFrame));
    assert_eq!(step.status, CommandStatus::Committed);
    let instance = session
        .editor_runtime_play_instance
        .as_ref()
        .expect("active runtime instance after step");
    assert_eq!(
        instance
            .runtime_world()
            .transform(&entity_id)
            .expect("gun transform after step")
            .local_position
            .x,
        9.0
    );

    let stop = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));
    assert_eq!(stop.status, CommandStatus::Committed);
    assert!(session.editor_runtime_play_instance.is_none());
    assert_eq!(session.selected_entity_id, None);
    assert_eq!(session.selected_entity_source, None);
    assert!(stop
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "runtime_temporary_edits_discarded"));
}

#[test]
fn runtime_apply_preview_and_confirm_updates_authoring_scene_and_stop_does_not_discard() {
    let package_dir = write_runtime_package_fixture();
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    attach_active_runtime_instance(&mut session, &package_dir);

    let select = session.execute_command(command_for_test(UiCommandPayload::SelectRuntimeEntity {
        entity_id: "entity-player".to_string(),
    }));
    assert_eq!(select.status, CommandStatus::Committed);
    let edit = session.execute_command(command_for_test(
        UiCommandPayload::SetRuntimeComponentFieldTemporary {
            entity_id: "entity-player".to_string(),
            component_type: "Transform".to_string(),
            field_path: "local_position.x".to_string(),
            value: serde_json::json!(4.0),
        },
    ));
    assert_eq!(edit.status, CommandStatus::Committed);

    let preview = session.execute_command(command_for_test(
        UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring,
    ));
    assert_eq!(preview.status, CommandStatus::Committed);
    let candidate = session
        .last_runtime_apply_report()
        .expect("runtime apply preview report")
        .candidates
        .first()
        .expect("candidate")
        .clone();
    assert_eq!(
        candidate.status,
        crate::ApplyRuntimeChangeCandidateStatus::Ready
    );
    assert_eq!(
        candidate.target_authoring_entity_id.as_deref(),
        Some("entity-player")
    );

    let apply = session.execute_command(command_for_test(
        UiCommandPayload::ApplyRuntimeChangeToAuthoring {
            edit_id: candidate.edit_id.clone(),
            candidate_hash: candidate.candidate_hash.clone(),
        },
    ));
    assert_eq!(apply.status, CommandStatus::Committed);
    let document = session
        .editor_scene_document
        .as_ref()
        .expect("authoring scene");
    assert!(document.dirty_state.dirty);
    assert_eq!(
        document
            .entity("entity-player")
            .expect("player")
            .transform
            .expect("transform")
            .local_position
            .x,
        4.0
    );
    assert_eq!(
        session
            .editor_runtime_play_instance
            .as_ref()
            .expect("active runtime")
            .temporary_edit_summary()
            .edited_field_count,
        0
    );

    let undo = session.execute_command(command_for_test(UiCommandPayload::UndoSceneEdit));
    assert_eq!(undo.status, CommandStatus::Committed);
    assert_eq!(
        session
            .editor_scene_document
            .as_ref()
            .expect("authoring scene")
            .entity("entity-player")
            .expect("player")
            .transform
            .expect("transform")
            .local_position
            .x,
        0.0
    );

    let stop = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));
    assert_eq!(stop.status, CommandStatus::Committed);
    assert!(!stop
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "runtime_temporary_edits_discarded"));
}

#[test]
fn runtime_apply_rejects_stale_candidate_hash() {
    let package_dir = write_runtime_package_fixture();
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    attach_active_runtime_instance(&mut session, &package_dir);
    session.execute_command(command_for_test(UiCommandPayload::SelectRuntimeEntity {
        entity_id: "entity-player".to_string(),
    }));
    let edit = session.execute_command(command_for_test(
        UiCommandPayload::SetRuntimeComponentFieldTemporary {
            entity_id: "entity-player".to_string(),
            component_type: "Transform".to_string(),
            field_path: "local_position.x".to_string(),
            value: serde_json::json!(4.0),
        },
    ));
    assert_eq!(edit.status, CommandStatus::Committed);
    let preview = session.execute_command(command_for_test(
        UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring,
    ));
    assert_eq!(preview.status, CommandStatus::Committed);
    let edit_id = session
        .last_runtime_apply_report()
        .expect("preview report")
        .candidates
        .first()
        .expect("candidate")
        .edit_id
        .clone();

    let apply = session.execute_command(command_for_test(
        UiCommandPayload::ApplyRuntimeChangeToAuthoring {
            edit_id,
            candidate_hash: "stale-hash".to_string(),
        },
    ));

    assert_eq!(apply.status, CommandStatus::Rejected);
    assert!(apply
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.runtime_apply.candidate_rejected"));
    assert!(session
        .last_runtime_apply_report()
        .expect("apply report")
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("stale_candidate_hash")));
}

#[test]
fn world_pick_reports_unsupported_when_world_has_no_supported_bounds() {
    let world = engine_runtime::world::World::new();
    let report = WorldPickCollector::pick(
        &world,
        RuntimeWorldPickRequest {
            x: 400.0,
            y: 300.0,
            viewport_width: Some(800.0),
            viewport_height: Some(600.0),
            aui_consumed: false,
        },
    );

    assert_eq!(report.status, RuntimePickStatus::Unsupported);
    assert_eq!(report.selected_entity_id, None);
    assert_eq!(report.diagnostic, "no_supported_runtime_bounds");
}

#[test]
fn select_missing_entity_returns_not_found_diagnostic() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);
    let result = session.execute_command(command_for_test(UiCommandPayload::SelectEntity {
        entity_id: "entity-missing".to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Rejected);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.entity.not_found"));
}

#[test]
fn tick_one_frame_updates_viewport_and_trace_model() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);
    let result = session.execute_command(command_for_test(UiCommandPayload::TickOneFrame));
    assert_eq!(result.status, CommandStatus::Committed);
    assert!(result
        .state_changes
        .iter()
        .any(|change| change.path == "runtime.frame"));
    let model = session.build_ui_model();
    assert_eq!(model.viewport.frame, 1);
    assert_eq!(model.viewport.renderable_count, 2);
    assert!(model
        .runtime_trace
        .entries
        .iter()
        .any(|entry| entry.system_id == "engine.frame_loop"));
}

#[test]
fn open_runtime_package_does_not_break_scene_document_open() {
    let package_dir = write_runtime_package_fixture();
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_session(&package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
        path: scene_path.display().to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    assert!(model.active_runtime_package.is_none());
    assert_eq!(model.hierarchy.scene_id.as_deref(), Some("scene-main"));
}

#[test]
fn open_runtime_package_does_not_mutate_editor_scene_document() {
    let package_dir = write_runtime_package_fixture();
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let before = session.editor_scene_document.clone();

    let result = session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
        path: package_dir.display().to_string(),
    }));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.editor_scene_document, before);
    assert!(session.build_ui_model().active_runtime_package.is_some());
}

fn attach_active_runtime_instance(session: &mut EditorSession, package_dir: &Path) {
    let _guard = RUNTIME_INSTANCE_START_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap();
    let request = EditorRuntimePlayRequest {
        schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
        session_id: "runtime-selection-test".to_string(),
        project_root: package_dir.to_path_buf(),
        runtime_package_path: package_dir.to_path_buf(),
        scene_ref: Some("scene-main".to_string()),
        run_profile: Some("editor-gameview".to_string()),
        frame_limit: 1,
        requested_by: "test".to_string(),
        preview_package_report_path: None,
    };
    let output = EditorRuntimePlayInstance::start(request);
    assert_eq!(
        output.present_report.status,
        GameViewPresentStatus::Success,
        "{:?}",
        output.present_report
    );
    session.editor_runtime_play_instance = output.instance;
    session.last_game_view_runtime_frame = output.frame;
    session.last_game_view_present_report = Some(output.present_report);
}
