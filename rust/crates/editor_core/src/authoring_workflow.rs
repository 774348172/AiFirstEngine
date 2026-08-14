use editor_ui_model::{
    AuiAuthoringAiContextSummary, AuthoringAiContext, AuthoringCommand,
    AuthoringCommandAvailability, AuthoringIssue, AuthoringIssueSeverity, AuthoringStepCompletion,
    AuthoringStepId, AuthoringStepStatus, AuthoringTask, AuthoringTaskPriority,
    AuthoringWorkflowModel, AuthoringWorkflowStep, PrefabAuthoringAiContextSummary,
    ProjectAuthoringWorkspaceModel, ProjectPatchAiContextSummary, WorkspaceDomainKind,
    WorkspaceDomainStatus, WorkspaceDomainSummary, AUTHORING_WORKFLOW_SCHEMA_VERSION,
};

pub struct AuthoringWorkflowComposer;

impl AuthoringWorkflowComposer {
    pub fn compose(workspace: &ProjectAuthoringWorkspaceModel) -> AuthoringWorkflowModel {
        if workspace.project_id.is_none() {
            return AuthoringWorkflowModel::empty();
        }

        let mut steps = AuthoringStepId::all()
            .into_iter()
            .map(|id| compose_step(id, workspace))
            .collect::<Vec<_>>();

        apply_cross_step_rules(&mut steps);

        let blocking_issues = steps
            .iter()
            .flat_map(|step| step.issues.iter())
            .filter(|issue| issue.blocks_play || issue.blocks_build)
            .cloned()
            .collect::<Vec<_>>();
        let recommended_tasks = recommended_tasks(&steps);
        let available_commands = steps
            .iter()
            .flat_map(|step| {
                step.primary_command
                    .iter()
                    .chain(step.secondary_commands.iter())
            })
            .filter(|command| command.availability == AuthoringCommandAvailability::Available)
            .cloned()
            .collect::<Vec<_>>();
        let missing_required_items = steps
            .iter()
            .filter(|step| {
                (step.is_required_for_play || step.is_required_for_build)
                    && matches!(
                        step.completion,
                        AuthoringStepCompletion::Missing | AuthoringStepCompletion::Blocked
                    )
            })
            .map(|step| step.id.as_str().to_string())
            .collect::<Vec<_>>();
        let can_play = can_play(&steps);
        let can_build = can_build(&steps);
        let active_step = first_actionable_step(&steps);
        let global_status = global_status(&steps, &blocking_issues);

        AuthoringWorkflowModel {
            schema_version: AUTHORING_WORKFLOW_SCHEMA_VERSION.to_string(),
            project_id: workspace.project_id.clone(),
            active_step,
            steps,
            global_status,
            can_play,
            can_build,
            blocking_issues: blocking_issues.clone(),
            recommended_tasks: recommended_tasks.clone(),
            ai_context: AuthoringAiContext {
                active_step,
                missing_required_items,
                blocking_issues,
                recommended_tasks,
                available_commands,
                manual_walkthrough_coverage: None,
                project_patch_summary: Some(project_patch_ai_context_summary()),
                prefab_authoring_summary: Some(prefab_authoring_ai_context_summary()),
                aui_authoring_summary: Some(aui_authoring_ai_context_summary()),
                summary: workflow_summary(global_status, can_play, can_build),
            },
        }
    }
}

fn aui_authoring_ai_context_summary() -> AuiAuthoringAiContextSummary {
    AuiAuthoringAiContextSummary {
        productized: true,
        scene_unified_authoring: true,
        visual_order_runtime_supported: true,
        visual_order_runtime_support_reason:
            "before_world_screen_overlay_modal_runtime_pass_supported; world_space_deferred"
                .to_string(),
        runtime_composition_gap_count: 0,
        next_required_runtime_gate: None,
        supported_commands: vec![
            "create_aui_document".to_string(),
            "open_aui_document".to_string(),
            "add_aui_node".to_string(),
            "set_aui_node_field".to_string(),
            "set_aui_binding_path".to_string(),
            "set_aui_action_ref".to_string(),
            "validate_aui_document".to_string(),
            "save_aui_document".to_string(),
            "preview_aui_overlay".to_string(),
        ],
        deferred_capabilities: vec![
            "aui_prefab_template_reuse".to_string(),
            "complex_scroll_input_controls".to_string(),
        ],
        next_actions: vec!["AUI Prefab / Template Reuse Productization v1".to_string()],
    }
}

fn prefab_authoring_ai_context_summary() -> PrefabAuthoringAiContextSummary {
    PrefabAuthoringAiContextSummary {
        productized: true,
        supported_commands: vec![
            "create_prefab_from_selection".to_string(),
            "open_prefab_document".to_string(),
            "enter_prefab_stage".to_string(),
            "exit_prefab_stage".to_string(),
            "instantiate_prefab_in_scene".to_string(),
            "set_prefab_stage_entity_field".to_string(),
            "apply_prefab_override_to_asset".to_string(),
            "save_prefab_document".to_string(),
            "validate_prefab_references".to_string(),
            "revert_prefab_override".to_string(),
        ],
        deferred_capabilities: vec![
            "nested_prefab".to_string(),
            "prefab_variant".to_string(),
            "batch_apply_revert".to_string(),
        ],
        next_actions: vec!["nested_prefab_or_prefab_variant_when_needed".to_string()],
    }
}

fn project_patch_ai_context_summary() -> ProjectPatchAiContextSummary {
    ProjectPatchAiContextSummary {
        productized: true,
        imported_patch_productized: true,
        llm_patch_source_available: true,
        active_patch_source_kind: "Mock".to_string(),
        supported_capabilities: vec![
            "Scene".to_string(),
            "Input".to_string(),
            "Asset".to_string(),
            "Prefab".to_string(),
            "Aui".to_string(),
            "Rule".to_string(),
            "Build".to_string(),
        ],
        unsupported_capabilities: Vec::new(),
        supported_import_sources: vec![
            "JsonString".to_string(),
            "FilePath".to_string(),
            "TestFixture".to_string(),
            "AiStructuredOutput".to_string(),
        ],
        imported_patch_commands: vec![
            "generate_project_patch_from_prompt".to_string(),
            "import_project_patch".to_string(),
            "preview_imported_project_patch".to_string(),
            "apply_imported_project_patch".to_string(),
        ],
        next_actions: vec![
            "use_generate_project_patch_from_prompt_for_ai_structured_output".to_string(),
            "use_project_patch_all_domain_a_min_schema".to_string(),
        ],
    }
}

fn compose_step(
    id: AuthoringStepId,
    workspace: &ProjectAuthoringWorkspaceModel,
) -> AuthoringWorkflowStep {
    let Some(domain) = domain(workspace, id.domain()) else {
        let mut step = AuthoringWorkflowStep::not_available(id);
        step.next_hint = Some(format!("{} domain is not configured.", id.label()));
        return step;
    };

    let status = step_status(domain.status);
    let completion = step_completion(status);
    let mut step = AuthoringWorkflowStep::new(id, status, completion);
    step.item_count = domain.item_count;
    step.primary_command = primary_command_for_step(id, domain);
    step.secondary_commands = secondary_commands_for_step(id, domain);
    step.issues = issues_for_domain(id, domain);
    step.next_hint = next_hint_for_step(id, domain, status);
    step
}

fn domain(
    workspace: &ProjectAuthoringWorkspaceModel,
    kind: WorkspaceDomainKind,
) -> Option<&WorkspaceDomainSummary> {
    workspace.domains.iter().find(|domain| domain.kind == kind)
}

fn step_status(status: WorkspaceDomainStatus) -> AuthoringStepStatus {
    match status {
        WorkspaceDomainStatus::NotConfigured => AuthoringStepStatus::NotAvailable,
        WorkspaceDomainStatus::Empty => AuthoringStepStatus::Empty,
        WorkspaceDomainStatus::Ready => AuthoringStepStatus::Ready,
        WorkspaceDomainStatus::Dirty => AuthoringStepStatus::Dirty,
        WorkspaceDomainStatus::Warning => AuthoringStepStatus::NeedsAttention,
        WorkspaceDomainStatus::Error => AuthoringStepStatus::Failed,
    }
}

fn step_completion(status: AuthoringStepStatus) -> AuthoringStepCompletion {
    match status {
        AuthoringStepStatus::NotAvailable
        | AuthoringStepStatus::Blocked
        | AuthoringStepStatus::Failed => AuthoringStepCompletion::Blocked,
        AuthoringStepStatus::Empty => AuthoringStepCompletion::Missing,
        AuthoringStepStatus::NeedsAttention | AuthoringStepStatus::Dirty => {
            AuthoringStepCompletion::Partial
        }
        AuthoringStepStatus::Running => AuthoringStepCompletion::Partial,
        AuthoringStepStatus::Ready => AuthoringStepCompletion::Ready,
        AuthoringStepStatus::Complete => AuthoringStepCompletion::Complete,
    }
}

fn apply_cross_step_rules(steps: &mut [AuthoringWorkflowStep]) {
    let scene_missing = steps.iter().any(|step| {
        step.id == AuthoringStepId::Scene
            && matches!(
                step.status,
                AuthoringStepStatus::NotAvailable
                    | AuthoringStepStatus::Empty
                    | AuthoringStepStatus::Blocked
                    | AuthoringStepStatus::Failed
            )
    });

    if scene_missing {
        block_step(
            steps,
            AuthoringStepId::Play,
            "authoring.play.blocked_by_scene",
            "Play needs an open editable scene.",
            true,
            false,
        );
        block_step(
            steps,
            AuthoringStepId::Build,
            "authoring.build.blocked_by_scene",
            "Build needs an open editable scene.",
            false,
            true,
        );
    }
}

fn block_step(
    steps: &mut [AuthoringWorkflowStep],
    id: AuthoringStepId,
    issue_id: &str,
    message: &str,
    blocks_play: bool,
    blocks_build: bool,
) {
    let Some(step) = steps.iter_mut().find(|step| step.id == id) else {
        return;
    };
    if matches!(
        step.status,
        AuthoringStepStatus::Ready | AuthoringStepStatus::Dirty | AuthoringStepStatus::Empty
    ) {
        step.status = AuthoringStepStatus::Blocked;
        step.completion = AuthoringStepCompletion::Blocked;
    }
    let mut issue = AuthoringIssue::new(
        issue_id,
        step.domain,
        AuthoringIssueSeverity::Error,
        message,
    );
    issue.blocks_play = blocks_play;
    issue.blocks_build = blocks_build;
    step.issues.push(issue);
}

fn issues_for_domain(id: AuthoringStepId, domain: &WorkspaceDomainSummary) -> Vec<AuthoringIssue> {
    let mut issues = Vec::new();
    if domain.diagnostics.error_count > 0 {
        let mut issue = AuthoringIssue::new(
            format!("authoring.{}.error", id.as_str()),
            domain.kind,
            AuthoringIssueSeverity::Error,
            format!(
                "{} has {} error(s).",
                id.label(),
                domain.diagnostics.error_count
            ),
        );
        issue.source_ref = domain.diagnostics.last_code.clone();
        issue.blocks_play = id.is_required_for_play();
        issue.blocks_build = id.is_required_for_build();
        issues.push(issue);
    }
    if domain.diagnostics.warning_count > 0 {
        let mut issue = AuthoringIssue::new(
            format!("authoring.{}.warning", id.as_str()),
            domain.kind,
            AuthoringIssueSeverity::Warning,
            format!(
                "{} has {} warning(s).",
                id.label(),
                domain.diagnostics.warning_count
            ),
        );
        issue.source_ref = domain.diagnostics.last_code.clone();
        issues.push(issue);
    }
    if id == AuthoringStepId::Scene && domain.status == WorkspaceDomainStatus::Empty {
        let mut issue = AuthoringIssue::new(
            "authoring.scene.missing",
            domain.kind,
            AuthoringIssueSeverity::Error,
            "Open or create a scene before play/build.",
        );
        issue.blocks_play = true;
        issue.blocks_build = true;
        issue.suggested_command = Some(AuthoringCommand::new(
            "open_scene_document",
            WorkspaceDomainKind::Scene,
            "Open Scene",
            AuthoringCommandAvailability::Available,
            "OpenSceneDocument",
        ));
        issues.push(issue);
    }
    issues
}

fn primary_command_for_step(
    id: AuthoringStepId,
    domain: &WorkspaceDomainSummary,
) -> Option<AuthoringCommand> {
    let available = command_availability(domain.status);
    match id {
        AuthoringStepId::Project => Some(AuthoringCommand::new(
            "open_project",
            WorkspaceDomainKind::Project,
            "Open Project",
            AuthoringCommandAvailability::Available,
            "OpenProject",
        )),
        AuthoringStepId::Assets => Some(AuthoringCommand::new(
            "select_project_browser_entry",
            WorkspaceDomainKind::Asset,
            "Browse Assets",
            available,
            "SelectProjectBrowserEntry",
        )),
        AuthoringStepId::Scene => Some(AuthoringCommand::new(
            "open_scene_document",
            WorkspaceDomainKind::Scene,
            "Open Scene",
            available,
            "OpenSceneDocument",
        )),
        AuthoringStepId::Input => Some(AuthoringCommand::new(
            "create_default_input_mapping",
            WorkspaceDomainKind::Input,
            "Create Input Mapping",
            AuthoringCommandAvailability::Available,
            "CreateDefaultInputMapping",
        )),
        AuthoringStepId::Play => Some(AuthoringCommand::new(
            "play",
            WorkspaceDomainKind::Play,
            "Play",
            available,
            "Play",
        )),
        AuthoringStepId::Build => Some(AuthoringCommand::new(
            "build_and_run_desktop_package",
            WorkspaceDomainKind::Build,
            "Build & Run",
            command_availability_for_build(domain.status),
            "BuildAndRunDesktopPackage",
        )),
        AuthoringStepId::Reports => Some(AuthoringCommand::new(
            "open_build_report",
            WorkspaceDomainKind::Report,
            "Open Report",
            command_availability(domain.status),
            "OpenBuildReport",
        )),
        AuthoringStepId::Prefabs => Some(AuthoringCommand::new(
            "open_prefab_document",
            WorkspaceDomainKind::Prefab,
            "Open Prefab",
            command_availability(domain.status),
            "OpenPrefabDocument",
        )),
        AuthoringStepId::Rules => Some(AuthoringCommand::new(
            if domain.item_count == 0 {
                "create_rule_asset"
            } else {
                "open_rule_asset"
            },
            WorkspaceDomainKind::Rule,
            if domain.item_count == 0 {
                "Create Rule"
            } else {
                "Open Rule"
            },
            command_availability(domain.status),
            if domain.item_count == 0 {
                "CreateRuleAsset"
            } else {
                "OpenRuleAsset"
            },
        )),
        AuthoringStepId::Aui => Some(AuthoringCommand::new(
            if domain.item_count == 0 {
                "create_aui_document"
            } else {
                "open_aui_document"
            },
            WorkspaceDomainKind::Aui,
            if domain.item_count == 0 {
                "Create AUI"
            } else {
                "Open AUI"
            },
            command_availability(domain.status),
            if domain.item_count == 0 {
                "CreateAuiDocument"
            } else {
                "OpenAuiDocument"
            },
        )),
    }
}

fn secondary_commands_for_step(
    id: AuthoringStepId,
    domain: &WorkspaceDomainSummary,
) -> Vec<AuthoringCommand> {
    match id {
        AuthoringStepId::Scene => vec![AuthoringCommand::new(
            "create_scene_entity",
            WorkspaceDomainKind::Scene,
            "Create Entity",
            command_availability(domain.status),
            "CreateSceneEntity",
        )],
        AuthoringStepId::Prefabs => {
            let availability = command_availability(domain.status);
            vec![
                AuthoringCommand::new(
                    "create_prefab_from_selection",
                    WorkspaceDomainKind::Prefab,
                    "Create Prefab",
                    availability,
                    "CreatePrefabFromSelection",
                ),
                AuthoringCommand::new(
                    "instantiate_prefab_in_scene",
                    WorkspaceDomainKind::Prefab,
                    "Instantiate Prefab",
                    availability,
                    "InstantiatePrefabInScene",
                ),
                AuthoringCommand::new(
                    "validate_prefab_references",
                    WorkspaceDomainKind::Prefab,
                    "Validate Prefabs",
                    availability,
                    "ValidatePrefabReferences",
                ),
            ]
        }
        AuthoringStepId::Reports => vec![AuthoringCommand::new(
            "clear_console",
            WorkspaceDomainKind::Report,
            "Clear Console",
            AuthoringCommandAvailability::Available,
            "ClearConsole",
        )],
        AuthoringStepId::Rules => {
            let availability = command_availability(domain.status);
            vec![
                AuthoringCommand::new(
                    "set_rule_card_field",
                    WorkspaceDomainKind::Rule,
                    "Edit Rule Card",
                    availability,
                    "SetRuleCardField",
                ),
                AuthoringCommand::new(
                    "add_rule_card",
                    WorkspaceDomainKind::Rule,
                    "Add Rule Card",
                    availability,
                    "AddRuleCard",
                ),
                AuthoringCommand::new(
                    "refresh_rule_graph_preview",
                    WorkspaceDomainKind::Rule,
                    "Refresh Rule Graph",
                    availability,
                    "RefreshRuleGraphPreview",
                ),
                AuthoringCommand::new(
                    "validate_rule_asset",
                    WorkspaceDomainKind::Rule,
                    "Validate Rule",
                    availability,
                    "ValidateRuleAsset",
                ),
                AuthoringCommand::new(
                    "build_rule_artifact",
                    WorkspaceDomainKind::Rule,
                    "Build Rule",
                    availability,
                    "BuildRuleArtifact",
                ),
            ]
        }
        AuthoringStepId::Aui => {
            let availability = command_availability(domain.status);
            vec![
                AuthoringCommand::new(
                    "add_aui_node",
                    WorkspaceDomainKind::Aui,
                    "Add AUI Node",
                    availability,
                    "AddAuiNode",
                ),
                AuthoringCommand::new(
                    "set_aui_binding_path",
                    WorkspaceDomainKind::Aui,
                    "Set AUI Binding",
                    availability,
                    "SetAuiBindingPath",
                ),
                AuthoringCommand::new(
                    "set_aui_action_ref",
                    WorkspaceDomainKind::Aui,
                    "Set AUI Action",
                    availability,
                    "SetAuiActionRef",
                ),
                AuthoringCommand::new(
                    "validate_aui_document",
                    WorkspaceDomainKind::Aui,
                    "Validate AUI",
                    availability,
                    "ValidateAuiDocument",
                ),
                AuthoringCommand::new(
                    "preview_aui_overlay",
                    WorkspaceDomainKind::Aui,
                    "Preview AUI",
                    availability,
                    "PreviewAuiOverlay",
                ),
                AuthoringCommand::new(
                    "save_aui_subtree_as_template",
                    WorkspaceDomainKind::Aui,
                    "Save AUI Template",
                    availability,
                    "SaveAuiSubtreeAsTemplate",
                ),
                AuthoringCommand::new(
                    "instantiate_aui_template",
                    WorkspaceDomainKind::Aui,
                    "Instantiate AUI Template",
                    availability,
                    "InstantiateAuiTemplate",
                ),
                AuthoringCommand::new(
                    "validate_aui_template",
                    WorkspaceDomainKind::Aui,
                    "Validate AUI Template",
                    availability,
                    "ValidateAuiTemplate",
                ),
            ]
        }
        _ => Vec::new(),
    }
}

fn command_availability(status: WorkspaceDomainStatus) -> AuthoringCommandAvailability {
    if status == WorkspaceDomainStatus::NotConfigured {
        AuthoringCommandAvailability::Disabled
    } else {
        AuthoringCommandAvailability::Available
    }
}

fn command_availability_for_build(status: WorkspaceDomainStatus) -> AuthoringCommandAvailability {
    match status {
        WorkspaceDomainStatus::NotConfigured | WorkspaceDomainStatus::Error => {
            AuthoringCommandAvailability::Disabled
        }
        WorkspaceDomainStatus::Empty
        | WorkspaceDomainStatus::Ready
        | WorkspaceDomainStatus::Dirty
        | WorkspaceDomainStatus::Warning => AuthoringCommandAvailability::Available,
    }
}

fn next_hint_for_step(
    id: AuthoringStepId,
    domain: &WorkspaceDomainSummary,
    status: AuthoringStepStatus,
) -> Option<String> {
    match status {
        AuthoringStepStatus::Empty => Some(match id {
            AuthoringStepId::Scene => "Open or create the main scene.".to_string(),
            AuthoringStepId::Input => "Create a default input mapping.".to_string(),
            AuthoringStepId::Rules => "Add project rules when gameplay needs logic.".to_string(),
            AuthoringStepId::Aui => "Add AUI documents when the game needs UI.".to_string(),
            AuthoringStepId::Play => "Load or build a runtime package before play.".to_string(),
            AuthoringStepId::Build => "Run export to produce a Windows package.".to_string(),
            _ => format!("Add {} content.", id.label()),
        }),
        AuthoringStepStatus::Dirty => Some(format!("Save {} changes.", id.label())),
        AuthoringStepStatus::NeedsAttention | AuthoringStepStatus::Failed => {
            Some(domain.summary.clone())
        }
        _ => None,
    }
}

fn recommended_tasks(steps: &[AuthoringWorkflowStep]) -> Vec<AuthoringTask> {
    steps
        .iter()
        .filter_map(|step| {
            let priority = match step.status {
                AuthoringStepStatus::Failed | AuthoringStepStatus::Blocked => {
                    AuthoringTaskPriority::Critical
                }
                AuthoringStepStatus::Empty if step.is_required_for_play => {
                    AuthoringTaskPriority::High
                }
                AuthoringStepStatus::NeedsAttention | AuthoringStepStatus::Dirty => {
                    AuthoringTaskPriority::Normal
                }
                AuthoringStepStatus::Empty => AuthoringTaskPriority::Low,
                _ => return None,
            };
            Some(AuthoringTask::new(
                format!("authoring.task.{}", step.id.as_str()),
                step.domain,
                priority,
                format!("Resolve {}", step.title),
                step.next_hint
                    .clone()
                    .unwrap_or_else(|| format!("Review {}.", step.title)),
                step.primary_command.clone(),
            ))
        })
        .collect()
}

fn first_actionable_step(steps: &[AuthoringWorkflowStep]) -> AuthoringStepId {
    steps
        .iter()
        .find(|step| {
            matches!(
                step.status,
                AuthoringStepStatus::Failed
                    | AuthoringStepStatus::Blocked
                    | AuthoringStepStatus::Empty
                    | AuthoringStepStatus::NeedsAttention
                    | AuthoringStepStatus::Dirty
            )
        })
        .map_or(AuthoringStepId::Project, |step| step.id)
}

fn can_play(steps: &[AuthoringWorkflowStep]) -> bool {
    let scene_ready = step_is_ready_for_action(steps, AuthoringStepId::Scene);
    let play_ready = step_is_ready_for_action(steps, AuthoringStepId::Play);
    let required_errors = steps.iter().any(|step| {
        step.is_required_for_play
            && matches!(
                step.status,
                AuthoringStepStatus::Failed | AuthoringStepStatus::Blocked
            )
    });
    scene_ready && play_ready && !required_errors
}

fn can_build(steps: &[AuthoringWorkflowStep]) -> bool {
    let scene_ready = step_is_ready_for_action(steps, AuthoringStepId::Scene);
    let build_available = steps.iter().any(|step| {
        step.id == AuthoringStepId::Build
            && matches!(
                step.status,
                AuthoringStepStatus::Empty
                    | AuthoringStepStatus::Ready
                    | AuthoringStepStatus::Dirty
                    | AuthoringStepStatus::NeedsAttention
            )
    });
    let required_errors = steps.iter().any(|step| {
        step.is_required_for_build
            && matches!(
                step.status,
                AuthoringStepStatus::Failed | AuthoringStepStatus::Blocked
            )
    });
    scene_ready && build_available && !required_errors
}

fn step_is_ready_for_action(steps: &[AuthoringWorkflowStep], id: AuthoringStepId) -> bool {
    steps.iter().any(|step| {
        step.id == id
            && matches!(
                step.status,
                AuthoringStepStatus::Ready
                    | AuthoringStepStatus::Dirty
                    | AuthoringStepStatus::NeedsAttention
            )
    })
}

fn global_status(
    steps: &[AuthoringWorkflowStep],
    blocking_issues: &[AuthoringIssue],
) -> AuthoringStepStatus {
    if blocking_issues
        .iter()
        .any(|issue| issue.severity == AuthoringIssueSeverity::Error)
    {
        return AuthoringStepStatus::Blocked;
    }
    if steps
        .iter()
        .any(|step| step.status == AuthoringStepStatus::Failed)
    {
        return AuthoringStepStatus::Failed;
    }
    if steps
        .iter()
        .any(|step| step.status == AuthoringStepStatus::Dirty)
    {
        return AuthoringStepStatus::Dirty;
    }
    if steps
        .iter()
        .any(|step| step.status == AuthoringStepStatus::NeedsAttention)
    {
        return AuthoringStepStatus::NeedsAttention;
    }
    AuthoringStepStatus::Ready
}

fn workflow_summary(status: AuthoringStepStatus, can_play: bool, can_build: bool) -> String {
    format!("status={status:?} can_play={can_play} can_build={can_build}")
}
