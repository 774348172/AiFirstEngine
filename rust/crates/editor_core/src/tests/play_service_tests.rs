use super::fixtures::*;
use super::*;

#[test]
fn editor_session_play_requires_runtime_package() {
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "editor.play_session.runtime_package_required" }));
}

#[test]
fn toolbar_play_enabled_for_open_project_without_runtime_package() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayProject".to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    assert!(session.runtime_package.is_none());

    let model = session.build_ui_model();
    let play = model
        .toolbar
        .commands
        .iter()
        .find(|command| command.command_id == "play")
        .expect("play command");

    assert!(play.enabled);
}

#[test]
fn editor_session_play_prepares_preview_package_for_open_project() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let result = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(result.status, CommandStatus::Committed);
    let preview = session
        .last_editor_preview_package_report()
        .expect("preview package report");
    assert_eq!(
        preview.schema_version,
        EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION
    );
    assert_eq!(preview.status, EditorPreviewPackageStatus::Success);
    assert_eq!(
        preview.cache_status,
        EditorPreviewPackageCacheStatus::Rebuilt
    );
    assert!(preview.runtime_package_dir.is_some());
    let play = session
        .last_play_session_report()
        .expect("play session report");
    assert_eq!(play.state, PlaySessionState::Running);
    assert_eq!(
        play.runner_kind.as_deref(),
        Some("editor_in_process_gameview")
    );
    assert_eq!(play.game_view_frame_count, Some(3));
    assert_eq!(play.preview_cache_status.as_deref(), Some("Rebuilt"));
    assert!(play.preview_package_report_path.is_some());
    let game_view = session
        .last_game_view_present_report()
        .expect("game view present report");
    assert_eq!(game_view.texture_descriptor_status, "descriptor_only");
    assert!(session.has_active_editor_runtime_play_instance());
    let model = session.build_ui_model();
    let frame = session
        .last_game_view_runtime_frame()
        .expect("game view runtime frame");
    assert_eq!(
        model.viewport.texture_id.as_deref(),
        Some(frame.texture_id.as_str())
    );
    assert_eq!(
        model.viewport.target_id.as_deref(),
        Some(frame.target_id.as_str())
    );
    assert!(model
        .report_panel
        .reports
        .iter()
        .any(|report| report.provider_id == "play.game_view_present"));
}

#[test]
fn toolbar_play_uses_the_session_game_view_target() {
    let mut session = EditorSession::new();
    let target = engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_720x1280();
    session.set_game_view_target(target);
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PortraitPreviewPlayProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(play.status, CommandStatus::Committed);
    assert_eq!(
        session
            .last_play_session_report()
            .expect("play session report")
            .request_summary
            .game_view_target,
        target
    );
    let frame = session
        .last_game_view_runtime_frame()
        .expect("portrait game view frame");
    assert_eq!((frame.width, frame.height), (720, 1280));
    assert_eq!(frame.presentation_scale_policy, target.scale_policy);
}

#[test]
fn project_settings_game_view_target_is_adopted_by_ordinary_editor_open() {
    let project_root = unique_editor_project_temp_dir();
    let mut creator = EditorSession::new();
    let create = creator.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PortraitProjectSettings".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    fs::write(
        project_root.join("Settings/project_settings.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": "aife-project-settings.v1",
            "projectName": "PortraitProjectSettings",
            "editorPreview": {
                "gameViewTarget": {
                    "extent": { "width": 720, "height": 1280 },
                    "scalePolicy": "contain"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut session = EditorSession::new();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));

    assert_eq!(open.status, CommandStatus::Committed);
    assert_eq!(
        session.game_view_target(),
        engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_720x1280()
    );
}

#[test]
fn game_view_target_command_updates_session_and_is_rejected_during_play() {
    let mut session = EditorSession::new();
    let set = session.execute_command(command_for_test(UiCommandPayload::SetGameViewTarget {
        width: 1080,
        height: 1920,
        scale_policy: editor_ui_model::EditorGameViewScalePolicy::Contain,
    }));
    assert_eq!(set.status, CommandStatus::Committed);
    assert_eq!(
        session.game_view_target(),
        engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_1080x1920()
    );
    let model = session.build_ui_model();
    assert_eq!(
        (
            model.toolbar.game_view_layout.target.width,
            model.toolbar.game_view_layout.target.height
        ),
        (1080, 1920)
    );
    assert!(model.toolbar.game_view_layout.target_editable);

    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "TargetLockedDuringPlay".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(play.status, CommandStatus::Committed);
    assert!(
        !session
            .build_ui_model()
            .toolbar
            .game_view_layout
            .target_editable
    );

    let rejected = session.execute_command(command_for_test(UiCommandPayload::SetGameViewTarget {
        width: 720,
        height: 1280,
        scale_policy: editor_ui_model::EditorGameViewScalePolicy::Contain,
    }));
    assert_eq!(rejected.status, CommandStatus::Rejected);
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.gameview_target.play_active"));
    assert_eq!(
        session.game_view_target(),
        engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_1080x1920()
    );
}

#[test]
fn editor_session_stop_cleans_gameview_runtime_instance() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayStopProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(play.status, CommandStatus::Committed);
    assert!(session.has_active_editor_runtime_play_instance());

    let stop = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));

    assert_eq!(stop.status, CommandStatus::Committed);
    assert!(!session.has_active_editor_runtime_play_instance());
    let report = session
        .last_game_view_present_report()
        .expect("game view stop report");
    assert_eq!(report.stop_status, "stopped");
}

#[test]
fn editor_session_pause_step_resume_controls_active_gameview_runtime() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayPauseStepProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(play.status, CommandStatus::Committed);
    let initial_frame = session
        .last_game_view_present_report()
        .expect("play report")
        .frame_count;

    let pause = session.execute_command(command_for_test(UiCommandPayload::Pause));
    assert_eq!(pause.status, CommandStatus::Committed);
    assert!(session.has_active_editor_runtime_play_instance());
    let paused = session
        .last_game_view_present_report()
        .expect("pause report");
    assert_eq!(paused.control_state, EditorRuntimePlayState::Paused);
    assert_eq!(paused.frame_count, initial_frame);
    assert!(!paused.runtime_advanced);

    let paused_tick = session
        .tick_active_game_view_runtime_descriptor_frame()
        .expect("paused redraw report");
    assert_eq!(paused_tick.frame_count, initial_frame);
    assert!(paused_tick.paused_last_frame_reused);
    assert!(!paused_tick.runtime_advanced);

    let step = session.execute_command(command_for_test(UiCommandPayload::StepFrame));
    assert_eq!(step.status, CommandStatus::Committed);
    let stepped = session
        .last_game_view_present_report()
        .expect("step report");
    assert_eq!(stepped.control_state, EditorRuntimePlayState::Paused);
    assert_eq!(stepped.frame_count, initial_frame + 1);
    assert_eq!(stepped.step_count, 1);
    assert_eq!(stepped.target_runtime_domain, "active_gameview_runtime");

    let resume = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(resume.status, CommandStatus::Committed);
    let resumed = session
        .last_game_view_present_report()
        .expect("resume report");
    assert_eq!(resumed.control_state, EditorRuntimePlayState::Running);
    assert_eq!(resumed.control_command, "resume");
    assert!(!resumed.runtime_advanced);
}

#[test]
fn editor_session_play_while_running_does_not_restart_gameview_runtime() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayAlreadyRunningProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(play.status, CommandStatus::Committed);
    let initial_frame = session
        .last_game_view_present_report()
        .expect("initial play report")
        .frame_count;

    let second_play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(second_play.status, CommandStatus::Committed);
    assert!(second_play
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.gameview_play.already_running"));
    assert_eq!(
        session
            .last_game_view_present_report()
            .expect("game view report should be unchanged")
            .frame_count,
        initial_frame
    );
    assert!(session.has_active_editor_runtime_play_instance());
}

#[test]
fn editor_session_maximize_on_play_is_editor_view_model_state() {
    let mut session = EditorSession::new();
    let project_root = unique_editor_project_temp_dir();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "PreviewPlayMaximizeProject".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);

    let set = session.execute_command(command_for_test(
        UiCommandPayload::SetGameViewMaximizeOnPlay { enabled: true },
    ));
    assert_eq!(set.status, CommandStatus::Committed);
    assert!(
        session
            .build_ui_model()
            .toolbar
            .game_view_layout
            .maximize_on_play
    );

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    assert_eq!(play.status, CommandStatus::Committed);
    let playing = session.build_ui_model();
    assert!(playing.toolbar.game_view_layout.maximize_on_play);
    assert!(playing.toolbar.game_view_layout.is_game_view_maximized);

    let stop = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));
    assert_eq!(stop.status, CommandStatus::Committed);
    let stopped = session.build_ui_model();
    assert!(stopped.toolbar.game_view_layout.maximize_on_play);
    assert!(!stopped.toolbar.game_view_layout.is_game_view_maximized);
}

#[test]
fn editor_session_play_runs_headless_session_when_package_loaded() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(result.status, CommandStatus::Committed);
    let report = session
        .play_session_controller
        .last_report()
        .expect("play session should produce report");
    assert_eq!(report.schema_version, PLAY_SESSION_REPORT_SCHEMA_VERSION);
    assert_eq!(report.state, PlaySessionState::Completed);
    assert_eq!(
        report
            .runtime_report
            .as_ref()
            .map(|runtime| runtime.frames_completed),
        Some(3)
    );
}

#[test]
fn editor_session_stop_without_active_session_reports_diagnostic() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));

    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "no_active_play_session"));
}

#[test]
fn editor_session_play_adds_console_summary() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(result.status, CommandStatus::Committed);
    let model = session.build_ui_model();
    assert!(model
        .console
        .entries
        .iter()
        .any(|entry| entry.message.contains("Play session completed")));
}

#[test]
fn editor_session_play_does_not_bypass_default_game_run_orchestrator() {
    let package_dir = write_runtime_package_fixture();
    let mut session = opened_session(&package_dir);

    let result = session.execute_command(command_for_test(UiCommandPayload::Play));

    assert_eq!(result.status, CommandStatus::Committed);
    let report = session.play_session_controller.last_report().unwrap();
    let runtime_report = report.runtime_report.as_ref().unwrap();
    assert_eq!(
        runtime_report.schema_version,
        "end-to-end-game-run-report.v1"
    );
    assert_eq!(runtime_report.package_load_status, "ok");
    assert_eq!(runtime_report.logic_tick_status, "ok");
    assert_eq!(runtime_report.present_status, "presented");
}
