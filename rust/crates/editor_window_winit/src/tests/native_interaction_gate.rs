use super::*;

#[test]
fn interaction_report_serializes() {
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.serialization",
        "Serialization",
    )
    .with_step(NativeEditorInteractionStep::click_hit_region(
        "click-missing",
        "missing",
    ));
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);
    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(NATIVE_EDITOR_INTERACTION_REPORT_SCHEMA_VERSION));
    assert!(json.contains("interaction.hit_region_missing"));
    assert_eq!(report.status, NativeEditorInteractionStatus::Failed);
}

#[test]
fn interaction_gate_create_project_enters_workspace() {
    let fixture_root = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(&fixture_root).expect("fixture owner root");
    let project_root = fixture_root.join("ScenarioCreated");
    let mut app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::default(),
        Box::new(HeadlessFolderDialogBackend::with_create_project_path(
            project_root.display().to_string(),
        )),
    );
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.create-project",
        "Create project from launcher",
    )
    .with_step(
        NativeEditorInteractionStep::click_hit_region(
            "click-create-project",
            "hit.project_launcher.create_project",
        )
        .expect_command("create_project", CommandStatus::Committed)
        .expect_mode(EditorUiMode::AuthoringWorkspace)
        .expect_revision_increase(),
    );

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);

    assert_eq!(
        report.status,
        NativeEditorInteractionStatus::Passed,
        "{report:#?}"
    );
    assert_eq!(report.final_mode, EditorUiMode::AuthoringWorkspace);
    assert!(project_root.join("project.aife.json").exists());
}

#[test]
fn interaction_gate_hierarchy_click_selects_entity_and_refreshes_inspector() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::with_session(
        NativeEditorWindowConfig::default(),
        opened_editor_project_session(&project_root),
    );
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.hierarchy-selection",
        "Hierarchy selection",
    )
    .with_step(
        NativeEditorInteractionStep::click_hit_region(
            "click-player",
            "hit.hierarchy.entity-player",
        )
        .expect_command("select_scene_entity", CommandStatus::Committed)
        .expect_selected_entity("entity-player")
        .expect_revision_increase(),
    );

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);

    assert_eq!(
        report.status,
        NativeEditorInteractionStatus::Passed,
        "{report:#?}"
    );
    assert_eq!(
        report.final_selected_entity_id.as_deref(),
        Some("entity-player")
    );
    assert_eq!(
        app.latest_model().inspector.selected_entity_id.as_deref(),
        Some("entity-player")
    );
}

#[test]
fn interaction_gate_inspector_field_edit_commits_transaction() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut session = opened_editor_project_session(&project_root);
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.inspector-edit",
        "Inspector property edit",
    )
    .with_step(NativeEditorInteractionStep::click_hit_region(
        "focus-local-position",
        "hit.inspector_field.transform.localPosition",
    ))
    .with_step(NativeEditorInteractionStep::replace_focused_property_text(
        "replace-local-position",
        "7,8,9",
    ))
    .with_step(
        NativeEditorInteractionStep::commit_focused_property_edit("commit-local-position")
            .expect_command("set_scene_transform", CommandStatus::Committed)
            .expect_revision_increase(),
    );

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);

    assert_eq!(
        report.status,
        NativeEditorInteractionStatus::Passed,
        "{report:#?}"
    );
    assert_eq!(app.transaction_service().committed_count, 1);
    assert_eq!(
        app.latest_model().viewport.renderables[0].local_position,
        editor_ui_model::Vec3 {
            x: 7.0,
            y: 8.0,
            z: 9.0,
        }
    );
}

#[test]
fn interaction_gate_ai_proposal_accept_routes_through_command_system() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut session = opened_editor_project_session(&project_root);
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::AiSubmitPrompt {
                    prompt: "rename selected to hero".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.ai-accept",
        "AI proposal accept",
    )
    .with_step(NativeEditorInteractionStep::click_hit_region(
        "open-ai-panel",
        "hit.dock_tab.ai_panel",
    ))
    .with_step(NativeEditorInteractionStep::frame("present-ai-panel"))
    .with_step(
        NativeEditorInteractionStep::click_hit_region(
            "accept-ai-proposal",
            format!("hit.ai_proposal.accept.{proposal_id}"),
        )
        .expect_command("ai_accept_proposed_command", CommandStatus::Committed)
        .expect_revision_increase(),
    );

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, NativeEditorInteractionStatus::Passed);
    assert_eq!(app.latest_model().hierarchy.roots[0].label, "hero");
    assert_eq!(app.transaction_service().committed_count, 1);
}

#[test]
fn interaction_gate_play_runs_for_open_project() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::with_session(
        NativeEditorWindowConfig::default(),
        opened_editor_project_session(&project_root),
    );
    let scenario = NativeEditorInteractionScenario::new(
        "native-editor.interaction.play-open-project",
        "Play open project",
    )
    .with_step(
        NativeEditorInteractionStep::click_hit_region("start-play", "hit.toolbar.play")
            .expect_command("play", CommandStatus::Pending),
    );

    let report = NativeEditorInteractionRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, NativeEditorInteractionStatus::Passed);
    assert_eq!(report.steps[0].command_id.as_deref(), Some("play"));
    assert_eq!(report.steps[0].command_status, Some(CommandStatus::Pending));
    assert_ne!(
        report.steps[0].feedback_status,
        Some(EditorCommandFeedbackStatus::Disabled)
    );
    assert!(report.diagnostics.is_empty());
    let completed = pump_editor_play_until_terminal(&mut app);
    assert_eq!(
        completed.last_command_status,
        Some(CommandStatus::Committed)
    );
    assert_eq!(
        app.session()
            .last_editor_preview_package_report()
            .expect("Play preparation report")
            .player_artifact_status,
        "not_required_in_process"
    );
}
