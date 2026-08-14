use super::fixtures::*;
use super::*;

#[test]
fn build_export_model_is_disabled_without_project() {
    let session = EditorSession::new();
    let model = session.build_ui_model();

    assert!(model.build_export.profiles.is_empty());
    assert!(model
        .build_export
        .commands
        .iter()
        .all(|command| !command.enabled));
    assert!(model.build_export.last_report.is_none());
    assert!(model
        .build_export
        .commands
        .iter()
        .any(
            |command| command.command_id == "build_and_run_desktop_package"
                && !command.enabled
                && command.reason_disabled.as_deref() == Some("Open a project first.")
        ));
}

#[test]
fn build_export_open_output_requires_prior_export() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_command(command_for_test(UiCommandPayload::OpenBuildOutput));

    assert_eq!(result.status, CommandStatus::Rejected);
    assert!(session.build_ui_model().console.unread_error_count > 0);
}

#[test]
fn build_export_desktop_package_updates_report_model() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result =
        session.execute_command(command_for_test(UiCommandPayload::ExportDesktopPackage {
            profile_id: Some("windows-dev".to_string()),
        }));
    let model = session.build_ui_model();
    let report = model
        .build_export
        .last_report
        .expect("desktop export should update report summary");

    assert!(matches!(
        result.status,
        CommandStatus::Committed | CommandStatus::Failed
    ));
    assert_eq!(
        model.build_export.selected_profile_id.as_deref(),
        Some("windows-dev")
    );
    assert!(
        report.package_dir.ends_with("Build\\Windows\\dev")
            || report.package_dir.ends_with("Build/Windows/dev")
    );
    assert!(
        report
            .report_path
            .ends_with("reports\\desktop-export-report.json")
            || report
                .report_path
                .ends_with("reports/desktop-export-report.json")
    );
    assert!(model
        .build_export
        .commands
        .iter()
        .any(|command| command.command_id == "open_build_report" && command.enabled));
}

#[test]
fn build_export_open_report_after_export_reports_path() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(UiCommandPayload::ExportDesktopPackage {
        profile_id: Some("windows-dev".to_string()),
    }));

    let result = session.execute_command(command_for_test(UiCommandPayload::OpenBuildReport));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(session
        .build_ui_model()
        .console
        .entries
        .iter()
        .any(|entry| entry.message.contains("Desktop export report:")));
}

#[test]
fn build_and_run_requires_project() {
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(
        UiCommandPayload::BuildAndRunDesktopPackage {
            profile_id: Some("windows-dev".to_string()),
        },
    ));

    assert_eq!(result.status, CommandStatus::Rejected);
    let report = session
        .last_build_and_run_report()
        .expect("rejected build and run should still produce in-memory report");
    assert_eq!(report.status, EditorBuildAndRunStatus::EnvironmentBlocked);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.build_and_run.no_project"));
}

#[test]
fn build_and_run_rejects_unsupported_profile() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_command(command_for_test(
        UiCommandPayload::BuildAndRunDesktopPackage {
            profile_id: Some("android-dev".to_string()),
        },
    ));

    assert_eq!(result.status, CommandStatus::Rejected);
    let report = session
        .last_build_and_run_report()
        .expect("unsupported profile should produce report");
    assert_eq!(report.status, EditorBuildAndRunStatus::EnvironmentBlocked);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.build_and_run.unsupported_profile"));
}

#[test]
fn build_and_run_desktop_package_updates_structured_report() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_command(command_for_test(
        UiCommandPayload::BuildAndRunDesktopPackage {
            profile_id: Some("windows-dev".to_string()),
        },
    ));

    assert!(matches!(
        result.status,
        CommandStatus::Committed | CommandStatus::Failed
    ));
    let report = session
        .last_build_and_run_report()
        .expect("build and run should update report")
        .clone();
    assert_eq!(
        report.schema_version,
        EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.run_mode, EditorBuildAndRunMode::UserWindowed);
    assert!(report.desktop_export.package_dir.is_some());
    assert!(report.report_path.as_ref().is_some_and(|path| {
        path.ends_with("reports\\editor-build-and-run-report.json")
            || path.ends_with("reports/editor-build-and-run-report.json")
    }));
    if let Some(path) = &report.report_path {
        assert!(std::path::Path::new(path).exists());
    }
    assert!(matches!(
        report.status,
        EditorBuildAndRunStatus::Launched
            | EditorBuildAndRunStatus::LaunchFailed
            | EditorBuildAndRunStatus::ExportFailed
    ));
}

#[test]
fn build_and_run_headless_verification_uses_staged_game_exe() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_build_and_run_desktop_package_for_test(
        Some("windows-dev".to_string()),
        EditorBuildAndRunMode::HeadlessVerification,
        30_000,
        2,
    );

    let report = session
        .last_build_and_run_report()
        .expect("headless build and run should produce report");
    assert_eq!(report.run_mode, EditorBuildAndRunMode::HeadlessVerification);
    assert_eq!(
        report.schema_version,
        EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION
    );
    assert!(report.launch.attempted);
    assert!(report.verification.stdout_summary.chars().count() <= 2_000);
    assert!(report.verification.stderr_summary.chars().count() <= 2_000);
    if report.status == EditorBuildAndRunStatus::VerificationPassed {
        assert_eq!(result.status, CommandStatus::Committed);
        assert_eq!(report.verification.child_player_exit_code, Some(0));
        assert!(report
            .verification
            .child_frames_completed
            .is_some_and(|frames| frames >= 2));
    } else {
        assert_eq!(result.status, CommandStatus::Failed);
        assert!(!report.diagnostics.is_empty());
    }
}

#[test]
fn build_service_release_profile_icon_picker_saves_with_source_guard() {
    let root = unique_editor_project_temp_dir();
    let project_root = root.join("project");
    copy_fixture_tree(&complex_shooter_project_root(), &project_root);
    let mut session = session_with_linked_project_runtime("sample.complex-shooter.runtime");
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));
    assert_eq!(open.status, CommandStatus::Committed);
    session.refresh_asset_browser_now("release-profile-picker-test");

    let initial = session.build_ui_model().build_export;
    let release = initial.release_profile.expect("release profile model");
    assert_eq!(release.display_name, "Complex Shooter");
    assert_eq!(release.icon_asset_id, "app-icon");
    assert!(initial
        .commands
        .iter()
        .any(|command| command.command_id == "build_release_package" && command.enabled));

    let begin = session.execute_command(command_for_test(UiCommandPayload::BeginAssetPick {
        field_id: "build.release.application.icon".to_string(),
    }));
    assert_eq!(begin.status, CommandStatus::Committed, "{begin:?}");
    let entry_key = session
        .build_ui_model()
        .asset_browser
        .entries
        .iter()
        .find(|entry| entry.path == "Assets/tex-player-ship.asset")
        .expect("texture picker candidate")
        .entry_key
        .clone();
    session.execute_command(command_for_test(
        UiCommandPayload::SelectAssetBrowserEntry {
            entry_key,
            additive: false,
            range: false,
        },
    ));
    let confirm = session.execute_command(command_for_test(UiCommandPayload::ConfirmAssetPick));
    assert_eq!(confirm.status, CommandStatus::Committed, "{confirm:?}");
    assert!(
        session
            .build_ui_model()
            .build_export
            .release_profile
            .unwrap()
            .dirty
    );
    let save = session.execute_command(command_for_test(UiCommandPayload::SaveReleaseProfile));
    assert_eq!(save.status, CommandStatus::Committed, "{save:?}");
    let saved: BuildProfile = serde_json::from_slice(
        &std::fs::read(project_root.join("BuildProfiles/windows.release.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved.application.unwrap().icon.asset_id, "tex-player-ship");

    let edit = session.execute_command(command_for_test(UiCommandPayload::SetReleaseProfileIcon {
        asset_ref: editor_ui_model::EditorAssetRef::new("app-icon", "texture"),
    }));
    assert_eq!(edit.status, CommandStatus::Committed);
    let profile_path = project_root.join("BuildProfiles/windows.release.json");
    let mut external = std::fs::read(&profile_path).unwrap();
    external.push(b' ');
    std::fs::write(&profile_path, external).unwrap();
    let rejected = session.execute_command(command_for_test(UiCommandPayload::SaveReleaseProfile));
    assert_eq!(rejected.status, CommandStatus::Rejected);
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "editor.release_profile.source_changed"));
}

#[test]
fn build_service_release_package_command_caches_atomic_report() {
    let root = unique_editor_project_temp_dir();
    let project_root = root.join("project");
    copy_fixture_tree(&complex_shooter_project_root(), &project_root);
    let mut session = session_with_linked_project_runtime("sample.complex-shooter.runtime");
    session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));
    let output_dir = root.join("release-final");

    let result = session.execute_build_release_package_for_test(
        crate::desktop_export::default_player_executable_for_project(&project_root)
            .expect("sample project declares a project Player"),
        output_dir.clone(),
        ReleasePackageReportLevel::Trace,
        false,
    );

    assert_eq!(result.status, CommandStatus::Committed, "{result:?}");
    let report = session
        .last_release_package_report()
        .expect("release report cache");
    assert_eq!(report.status, ReleasePackageStatus::Success);
    assert_eq!(report.entrypoint, "ComplexShooter.exe");
    assert!(report.verification.runtime_load_passed);
    assert!(project_root
        .join(RELEASE_PACKAGE_REPORT_RELATIVE_PATH)
        .is_file());
    assert!(output_dir.join("ComplexShooter.exe").is_file());
}

fn complex_shooter_project_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("samples")
        .join("complex_shooter_project")
}

fn copy_fixture_tree(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture_tree(&entry.path(), &destination_path);
        } else {
            std::fs::copy(entry.path(), destination_path).unwrap();
        }
    }
}
