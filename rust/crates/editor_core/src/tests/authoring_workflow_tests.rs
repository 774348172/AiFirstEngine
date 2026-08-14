use super::fixtures::*;
use super::*;

#[test]
fn authoring_workflow_blocks_until_project_is_open() {
    let session = EditorSession::new();
    let workflow = session.build_ui_model().authoring_workflow;

    assert_eq!(
        workflow.project_id.as_deref(),
        None,
        "empty session should not invent a project"
    );
    assert_eq!(
        workflow.active_step,
        editor_ui_model::AuthoringStepId::Project
    );
    assert!(!workflow.can_play);
    assert!(!workflow.can_build);
    assert!(workflow
        .ai_context
        .missing_required_items
        .contains(&"project".to_string()));
}

#[test]
fn authoring_workflow_summarizes_created_project_from_workspace_domains() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();

    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let workflow = session.build_ui_model().authoring_workflow;
    assert!(workflow
        .project_id
        .as_deref()
        .is_some_and(|id| id.starts_with("project-")));
    assert_eq!(
        workflow
            .step(editor_ui_model::AuthoringStepId::Project)
            .expect("project step")
            .status,
        editor_ui_model::AuthoringStepStatus::Ready
    );
    assert_eq!(
        workflow
            .step(editor_ui_model::AuthoringStepId::Scene)
            .expect("scene step")
            .status,
        editor_ui_model::AuthoringStepStatus::Ready
    );
    assert!(workflow
        .steps
        .iter()
        .any(|step| step.id == editor_ui_model::AuthoringStepId::Build));
    assert!(workflow.can_build);
}

#[test]
fn authoring_workflow_tracks_optional_domains_without_game_specific_rules() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    std::fs::create_dir_all(root.join("Rules")).unwrap();
    std::fs::create_dir_all(root.join("AUI")).unwrap();
    std::fs::create_dir_all(root.join("Input")).unwrap();
    std::fs::write(root.join("Rules").join("frame.rule.json"), "{}").unwrap();
    std::fs::write(root.join("AUI").join("hud.aui.json"), "{}").unwrap();
    std::fs::write(
        root.join("Input").join("game.input-mapping.json"),
        serde_json::to_string_pretty(&engine_input::InputMappingAsset::gameplay_default()).unwrap(),
    )
    .unwrap();

    let workflow = session.build_ui_model().authoring_workflow;

    for (step_id, expected_count) in [
        (editor_ui_model::AuthoringStepId::Rules, 1),
        (editor_ui_model::AuthoringStepId::Input, 2),
        (editor_ui_model::AuthoringStepId::Aui, 1),
    ] {
        let step = workflow.step(step_id).expect("workflow step should exist");
        assert_eq!(step.status, editor_ui_model::AuthoringStepStatus::Ready);
        assert_eq!(step.item_count, expected_count);
    }
    assert!(!workflow
        .recommended_tasks
        .iter()
        .any(|task| task.title.contains("Player")
            || task.title.contains("Enemy")
            || task.title.contains("Bullet")));
}

#[test]
fn authoring_workflow_active_step_is_editor_ui_state() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_command(command_for_test(
        UiCommandPayload::SetAuthoringWorkflowStep {
            step_id: editor_ui_model::AuthoringStepId::Build,
        },
    ));
    let workflow = session.build_ui_model().authoring_workflow;

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(
        workflow.active_step,
        editor_ui_model::AuthoringStepId::Build
    );
    assert_eq!(
        workflow.ai_context.active_step,
        editor_ui_model::AuthoringStepId::Build
    );
    assert!(result.state_changes.iter().any(|change| {
        change.path == "authoring_workflow.active_step"
            && change.after_summary.as_deref() == Some("build")
    }));
}

#[test]
fn authoring_workflow_exposes_core_domain_commands() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let workflow = session.build_ui_model().authoring_workflow;
    for (step_id, expected_command_id) in [
        (editor_ui_model::AuthoringStepId::Project, "open_project"),
        (
            editor_ui_model::AuthoringStepId::Scene,
            "open_scene_document",
        ),
        (
            editor_ui_model::AuthoringStepId::Input,
            "create_default_input_mapping",
        ),
        (editor_ui_model::AuthoringStepId::Play, "play"),
        (
            editor_ui_model::AuthoringStepId::Build,
            "build_and_run_desktop_package",
        ),
        (
            editor_ui_model::AuthoringStepId::Reports,
            "open_build_report",
        ),
    ] {
        let step = workflow.step(step_id).expect("workflow step");
        assert_eq!(
            step.primary_command
                .as_ref()
                .expect("primary command")
                .command_id,
            expected_command_id
        );
    }
}

#[test]
fn authoring_workflow_available_commands_exclude_disabled_domains() {
    let workflow = EditorSession::new().build_ui_model().authoring_workflow;

    assert!(workflow
        .ai_context
        .available_commands
        .iter()
        .all(|command| command.availability
            == editor_ui_model::AuthoringCommandAvailability::Available));
    assert!(!workflow
        .ai_context
        .available_commands
        .iter()
        .any(|command| command.command_id == "build_and_run_desktop_package"));
}

#[test]
fn authoring_workflow_ai_context_matches_workflow_state_after_command() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::SetAuthoringWorkflowStep {
            step_id: editor_ui_model::AuthoringStepId::Input,
        },
    ));

    let workflow = session.build_ui_model().authoring_workflow;

    assert_eq!(workflow.ai_context.active_step, workflow.active_step);
    assert_eq!(
        workflow.ai_context.recommended_tasks,
        workflow.recommended_tasks
    );
    assert_eq!(
        workflow.ai_context.blocking_issues,
        workflow.blocking_issues
    );
}

#[test]
fn manual_walkthrough_coverage_classifies_current_command_surface() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let model = session.build_ui_model();
    let report = ManualWalkthroughCoverageAnalyzer::analyze(ManualWalkthroughCoverageInput {
        workspace: &model.project_authoring_workspace,
        workflow: &model.authoring_workflow,
        scenario_id: "manual-walkthrough-test",
    });

    assert_eq!(
        report.status,
        editor_ui_model::ManualWalkthroughCoverageStatus::Partial
    );
    assert!(report.operation_count >= 60);
    assert_operation_status(
        &report,
        "play",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommand,
    );
    assert_operation_status(
        &report,
        "export_desktop_package",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommand,
    );
    assert_operation_status(
        &report,
        "build_and_run_desktop_package",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommand,
    );
    assert_operation_status(
        &report,
        "clear_console",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommand,
    );
    assert_operation_status(
        &report,
        "open_scene_document",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "open_prefab_document",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "edit_rule_graph_or_dsl",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "add_aui_node",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "generate_project_patch_from_prompt",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "preview_imported_project_patch",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert_operation_status(
        &report,
        "apply_imported_project_patch",
        editor_ui_model::ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
    );
    assert!(!report.next_actions.is_empty());
}

#[test]
fn authoring_workflow_ai_context_exposes_manual_walkthrough_summary() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let workflow = session.build_ui_model().authoring_workflow;
    let summary = workflow
        .ai_context
        .manual_walkthrough_coverage
        .expect("coverage summary");

    assert_eq!(
        summary.status,
        editor_ui_model::ManualWalkthroughCoverageStatus::Partial
    );
    assert!(summary.operation_count >= 60);
    assert!(summary.missing_command_count > 0);
    assert!(!summary.top_next_actions.is_empty());

    let patch_summary = workflow
        .ai_context
        .project_patch_summary
        .expect("ProjectPatch summary");
    assert!(patch_summary.productized);
    assert!(patch_summary.imported_patch_productized);
    assert!(patch_summary.llm_patch_source_available);
    assert_eq!(patch_summary.active_patch_source_kind, "Mock");
    assert_eq!(
        patch_summary.supported_capabilities,
        vec![
            "Scene".to_string(),
            "Input".to_string(),
            "Asset".to_string(),
            "Prefab".to_string(),
            "Aui".to_string(),
            "Rule".to_string(),
            "Build".to_string()
        ]
    );
    assert!(patch_summary
        .imported_patch_commands
        .contains(&"generate_project_patch_from_prompt".to_string()));
    assert!(patch_summary
        .imported_patch_commands
        .contains(&"preview_imported_project_patch".to_string()));
    assert!(patch_summary
        .supported_import_sources
        .contains(&"JsonString".to_string()));
    assert!(patch_summary.unsupported_capabilities.is_empty());
    assert!(!patch_summary.next_actions.is_empty());

    let prefab_summary = workflow
        .ai_context
        .prefab_authoring_summary
        .expect("Prefab authoring summary");
    assert!(prefab_summary.productized);
    assert!(prefab_summary
        .supported_commands
        .contains(&"enter_prefab_stage".to_string()));
    assert!(!prefab_summary
        .deferred_capabilities
        .contains(&"project_patch_prefab_capability_v2".to_string()));

    let aui_summary = workflow
        .ai_context
        .aui_authoring_summary
        .expect("AUI authoring summary");
    assert!(aui_summary.productized);
    assert!(aui_summary.scene_unified_authoring);
    assert!(aui_summary.visual_order_runtime_supported);
    assert_eq!(aui_summary.runtime_composition_gap_count, 0);
    assert!(aui_summary.next_required_runtime_gate.is_none());
    assert!(aui_summary
        .supported_commands
        .contains(&"preview_aui_overlay".to_string()));
    assert!(!aui_summary
        .deferred_capabilities
        .contains(&"runtime_multi_stage_ui_composition_pass".to_string()));
}

fn assert_operation_status(
    report: &editor_ui_model::ManualWalkthroughCoverageReport,
    operation_id: &str,
    status: editor_ui_model::ManualAuthoringOperationStatus,
) {
    let operation = report
        .operations
        .iter()
        .find(|operation| operation.requirement.operation_id == operation_id)
        .unwrap_or_else(|| panic!("missing operation {operation_id}"));
    assert_eq!(
        operation.status, status,
        "unexpected status for {operation_id}: {}",
        operation.resolution_summary
    );
}
