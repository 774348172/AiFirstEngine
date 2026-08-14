use editor_ui_model::{
    manual_authoring_operation_requirements, AuthoringCommand, AuthoringCommandAvailability,
    AuthoringWorkflowModel, ManualAuthoringOperationRequirement, ManualAuthoringOperationStatus,
    ManualWalkthroughCoverageReport, ManualWalkthroughOperationCoverage,
    ManualWalkthroughRequiredContext, ProjectAuthoringWorkspaceModel, UiCommandPayload,
    WorkflowCommandResolution, WorkflowCommandResolver, WorkspaceDomainKind, WorkspaceDomainStatus,
};

pub struct ManualWalkthroughCoverageAnalyzer;

#[derive(Debug, Clone)]
pub struct ManualWalkthroughCoverageInput<'a> {
    pub workspace: &'a ProjectAuthoringWorkspaceModel,
    pub workflow: &'a AuthoringWorkflowModel,
    pub scenario_id: &'a str,
}

impl ManualWalkthroughCoverageAnalyzer {
    pub fn analyze(input: ManualWalkthroughCoverageInput<'_>) -> ManualWalkthroughCoverageReport {
        let operations = manual_authoring_operation_requirements()
            .into_iter()
            .map(|requirement| analyze_requirement(&requirement, &input))
            .collect::<Vec<_>>();
        let diagnostics = if input.workspace.project_id.is_none() {
            vec!["fail:no_project_open".to_string()]
        } else {
            Vec::new()
        };

        ManualWalkthroughCoverageReport::from_operations(
            input.workspace.project_id.clone(),
            input.scenario_id,
            operations,
            diagnostics,
        )
    }
}

fn analyze_requirement(
    requirement: &ManualAuthoringOperationRequirement,
    input: &ManualWalkthroughCoverageInput<'_>,
) -> ManualWalkthroughOperationCoverage {
    if input.workspace.project_id.is_none()
        && !matches!(
            requirement.required_context.as_slice(),
            [ManualWalkthroughRequiredContext::None]
        )
    {
        return coverage(
            requirement,
            ManualAuthoringOperationStatus::MissingDomainService,
            "No project is open, so this domain summary is unavailable.",
            Some("open_project"),
        );
    }

    let domain_status = domain_status(input.workspace, requirement.domain);
    if domain_status == Some(WorkspaceDomainStatus::NotConfigured)
        && requirement.required_for_complex_project
    {
        return coverage(
            requirement,
            ManualAuthoringOperationStatus::MissingDomainService,
            "Domain summary is not configured.",
            Some(suggest_next_action(requirement.domain)),
        );
    }

    let Some(command_id) = requirement.expected_command_id.as_deref() else {
        return classify_without_command(requirement, input);
    };

    if requirement.operation_id == "ai_project_patch_scene_input" {
        return coverage(
            requirement,
            ManualAuthoringOperationStatus::ExecutableCommand,
            "ProjectPatch All-Domain Capability v2 keeps Scene/Input review/apply/report executable through EditorSession::execute_patch_as_transaction.",
            None::<String>,
        );
    }

    let command = workflow_command_for_requirement(requirement, input, command_id);
    match WorkflowCommandResolver::resolve(&command) {
        WorkflowCommandResolution::Command(payload) => {
            if payload_needs_context(&payload) {
                coverage(
                    requirement,
                    ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
                    format!(
                        "{} resolves to {}, but required context is incomplete.",
                        command_id,
                        payload_kind(&payload)
                    ),
                    Some(context_next_action(requirement)),
                )
            } else {
                coverage(
                    requirement,
                    ManualAuthoringOperationStatus::ExecutableCommand,
                    format!(
                        "{} resolves to executable {}.",
                        command_id,
                        payload_kind(&payload)
                    ),
                    None::<String>,
                )
            }
        }
        WorkflowCommandResolution::FocusDomainPanel { reason, .. } => coverage(
            requirement,
            ManualAuthoringOperationStatus::FocusDomainPanel,
            reason,
            Some(context_next_action(requirement)),
        ),
        WorkflowCommandResolution::Disabled { reason } => coverage(
            requirement,
            ManualAuthoringOperationStatus::BlockedByDependency,
            reason,
            Some(suggest_next_action(requirement.domain)),
        ),
        WorkflowCommandResolution::Unsupported { reason } => coverage(
            requirement,
            ManualAuthoringOperationStatus::MissingCommand,
            reason,
            Some(suggest_next_action(requirement.domain)),
        ),
    }
}

fn classify_without_command(
    requirement: &ManualAuthoringOperationRequirement,
    input: &ManualWalkthroughCoverageInput<'_>,
) -> ManualWalkthroughOperationCoverage {
    if requirement.operation_id == "ai_project_patch_asset_prefab_aui_rule" {
        return coverage(
            requirement,
            ManualAuthoringOperationStatus::ExecutableCommand,
            "ProjectPatch All-Domain Capability v2 supports Asset/Prefab/AUI/Rule/Build A-min operations through schema validation, review, transaction apply, and report.",
            None::<String>,
        );
    }

    if domain_status(input.workspace, requirement.domain).is_none() {
        return coverage(
            requirement,
            ManualAuthoringOperationStatus::MissingDomainService,
            "Domain summary is missing.",
            Some(suggest_next_action(requirement.domain)),
        );
    }

    coverage(
        requirement,
        ManualAuthoringOperationStatus::MissingCommand,
        "No formal UiCommandPayload / AuthoringCommand entry exists.",
        Some(suggest_next_action(requirement.domain)),
    )
}

fn workflow_command_for_requirement(
    requirement: &ManualAuthoringOperationRequirement,
    input: &ManualWalkthroughCoverageInput<'_>,
    command_id: &str,
) -> AuthoringCommand {
    let availability = input
        .workflow
        .steps
        .iter()
        .flat_map(|step| {
            step.primary_command
                .iter()
                .chain(step.secondary_commands.iter())
        })
        .find(|command| command.command_id == command_id)
        .map_or(AuthoringCommandAvailability::Available, |command| {
            command.availability
        });

    AuthoringCommand::new(
        command_id,
        requirement.domain,
        requirement.title.clone(),
        availability,
        requirement
            .expected_payload_kind
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
    )
}

fn coverage(
    requirement: &ManualAuthoringOperationRequirement,
    status: ManualAuthoringOperationStatus,
    resolution_summary: impl Into<String>,
    next_action: Option<impl Into<String>>,
) -> ManualWalkthroughOperationCoverage {
    let gap_id = (!matches!(
        status,
        ManualAuthoringOperationStatus::ExecutableCommand
            | ManualAuthoringOperationStatus::FocusDomainPanel
            | ManualAuthoringOperationStatus::Deferred
    ))
    .then(|| format!("gap.{}", requirement.operation_id));

    ManualWalkthroughOperationCoverage {
        requirement: requirement.clone(),
        status,
        resolution_summary: resolution_summary.into(),
        next_action: next_action.map(Into::into),
        gap_id,
        diagnostics: Vec::new(),
    }
}

fn domain_status(
    workspace: &ProjectAuthoringWorkspaceModel,
    domain: WorkspaceDomainKind,
) -> Option<WorkspaceDomainStatus> {
    workspace
        .domains
        .iter()
        .find(|summary| summary.kind == domain)
        .map(|summary| summary.status)
}

fn payload_needs_context(payload: &UiCommandPayload) -> bool {
    match payload {
        UiCommandPayload::OpenProject { path }
        | UiCommandPayload::CreateDefaultInputMapping { path }
        | UiCommandPayload::DeleteInputMapping { path }
        | UiCommandPayload::OpenRuleAsset { path }
        | UiCommandPayload::SelectRuleAsset { path }
        | UiCommandPayload::ValidateRuleAsset { path }
        | UiCommandPayload::BuildRuleArtifact { path }
        | UiCommandPayload::BuildProjectRuleManifest { path }
        | UiCommandPayload::SaveRuleAsset { path }
        | UiCommandPayload::OpenRuleDiagnostics { path }
        | UiCommandPayload::RefreshRuleGraphPreview { path }
        | UiCommandPayload::OpenPrefabDocument { path }
        | UiCommandPayload::SavePrefabDocument { path }
        | UiCommandPayload::OpenRuntimePackage { path }
        | UiCommandPayload::OpenSceneDocument { path }
        | UiCommandPayload::SelectProjectBrowserEntry { path }
        | UiCommandPayload::OpenProjectBrowserEntry { path }
        | UiCommandPayload::OpenInputMapping { path }
        | UiCommandPayload::SaveInputMapping { path }
        | UiCommandPayload::DiscardInputMappingDraft { path }
        | UiCommandPayload::ValidateInputMapping { path } => path.is_empty(),
        UiCommandPayload::SelectAuiNode {
            document_path,
            document_id,
            node_id,
        } => document_path.is_empty() || document_id.is_empty() || node_id.is_empty(),
        UiCommandPayload::RegisterExistingAsset { path, .. } => path.is_empty(),
        UiCommandPayload::GenerateMockImageAsset {
            prompt,
            target_folder,
            asset_name,
            image_kind,
            width,
            height,
            ..
        } => {
            prompt.is_empty()
                || target_folder.is_empty()
                || asset_name.is_empty()
                || image_kind.is_empty()
                || *width == 0
                || *height == 0
        }
        UiCommandPayload::ValidateAssetBrowserIndex { .. } => false,
        UiCommandPayload::SaveSceneDocument { path } => path.as_deref().is_some_and(str::is_empty),
        UiCommandPayload::SelectEntity { entity_id }
        | UiCommandPayload::SelectRuntimeEntity { entity_id }
        | UiCommandPayload::SelectSceneEntity { entity_id }
        | UiCommandPayload::DeleteSceneEntity { entity_id }
        | UiCommandPayload::RenameSceneEntity { entity_id, .. } => entity_id.is_empty(),
        UiCommandPayload::CreateSceneEntity { name, .. } => name.is_empty(),
        UiCommandPayload::PlaceAssetIntoScene { asset_id, .. } => asset_id.is_empty(),
        UiCommandPayload::SetSceneTransform { entity_id, .. }
        | UiCommandPayload::AddSceneComponent { entity_id, .. }
        | UiCommandPayload::RemoveSceneComponent { entity_id, .. }
        | UiCommandPayload::SetSceneComponentField { entity_id, .. }
        | UiCommandPayload::SetRuntimeComponentFieldTemporary { entity_id, .. } => {
            entity_id.is_empty()
        }
        UiCommandPayload::ApplyRuntimeChangeToAuthoring {
            edit_id,
            candidate_hash,
        } => edit_id.is_empty() || candidate_hash.is_empty(),
        UiCommandPayload::CreatePrefabFromSelection {
            root_entity_id,
            prefab_id,
            name,
            ..
        } => root_entity_id.is_empty() || prefab_id.is_empty() || name.is_empty(),
        UiCommandPayload::EnterPrefabStage { path, .. } => path.is_empty(),
        UiCommandPayload::ExitPrefabStage { .. } => false,
        UiCommandPayload::InstantiatePrefabInScene { prefab_id, .. } => prefab_id.is_empty(),
        UiCommandPayload::SetPrefabStageEntityField {
            source_entity_id,
            field_path,
            value,
            ..
        } => source_entity_id.is_empty() || field_path.is_empty() || value.is_null(),
        UiCommandPayload::ApplyPrefabOverrideToAsset {
            instance_entity_id,
            target_source_entity_id,
            component_type,
            field_path,
        }
        | UiCommandPayload::RevertPrefabOverride {
            instance_entity_id,
            target_source_entity_id,
            component_type,
            field_path,
        } => {
            instance_entity_id.is_empty()
                || target_source_entity_id.is_empty()
                || component_type.is_empty()
                || field_path.is_empty()
        }
        UiCommandPayload::ValidatePrefabReferences { path } => {
            path.as_deref().is_some_and(str::is_empty)
        }
        UiCommandPayload::CreateAuiDocument {
            path,
            document_id,
            width,
            height,
        } => path.is_empty() || document_id.is_empty() || *width <= 0.0 || *height <= 0.0,
        UiCommandPayload::OpenAuiDocument { path }
        | UiCommandPayload::ValidateAuiDocument { path }
        | UiCommandPayload::SaveAuiDocument { path }
        | UiCommandPayload::PreviewAuiOverlay { path } => path.is_empty(),
        UiCommandPayload::AddAuiNode {
            path,
            parent_node_id,
            node_id,
            kind,
            rect,
            ..
        } => {
            path.is_empty()
                || parent_node_id.is_empty()
                || node_id.is_empty()
                || kind.is_empty()
                || rect.is_null()
        }
        UiCommandPayload::SetAuiNodeField {
            path,
            node_id,
            schema_path,
            value,
        } => path.is_empty() || node_id.is_empty() || schema_path.is_empty() || value.is_null(),
        UiCommandPayload::SetAuiBindingPath {
            path,
            node_id,
            target_field,
            binding_id,
            binding_path,
            ..
        } => {
            path.is_empty()
                || node_id.is_empty()
                || target_field.is_empty()
                || binding_id.is_empty()
                || binding_path.is_empty()
        }
        UiCommandPayload::SetAuiActionRef {
            path,
            node_id,
            event,
            action_id,
            ..
        } => path.is_empty() || node_id.is_empty() || event.is_empty() || action_id.is_empty(),
        UiCommandPayload::SaveAuiSubtreeAsTemplate {
            document_path,
            root_node_id,
            template_asset_path,
            template_id,
            ..
        } => {
            document_path.is_empty()
                || root_node_id.is_empty()
                || template_asset_path.is_empty()
                || template_id.is_empty()
        }
        UiCommandPayload::InstantiateAuiTemplate {
            template_asset_path,
            template_id,
            target_document_path,
            parent_node_id,
            instance_id,
            ..
        } => {
            template_asset_path.is_empty()
                || template_id.is_empty()
                || target_document_path.is_empty()
                || parent_node_id.is_empty()
                || instance_id.is_empty()
        }
        UiCommandPayload::ValidateAuiTemplate {
            template_asset_path,
            template_id,
        } => template_asset_path.is_empty() || template_id.is_empty(),
        UiCommandPayload::AddInputAction {
            path, action_id, ..
        }
        | UiCommandPayload::RemoveInputAction { path, action_id }
        | UiCommandPayload::SetInputActionValueType {
            path, action_id, ..
        }
        | UiCommandPayload::SelectInputAction { path, action_id } => {
            path.is_empty() || action_id.is_empty()
        }
        UiCommandPayload::AddInputContext {
            path, context_id, ..
        }
        | UiCommandPayload::RemoveInputContext { path, context_id }
        | UiCommandPayload::SetInputContextPriority {
            path, context_id, ..
        }
        | UiCommandPayload::SetInputContextConsumeInput {
            path, context_id, ..
        }
        | UiCommandPayload::SelectInputContext { path, context_id } => {
            path.is_empty() || context_id.is_empty()
        }
        UiCommandPayload::AddInputBinding {
            path,
            context_id,
            action_id,
            device_path,
        } => {
            path.is_empty()
                || context_id.is_empty()
                || action_id.is_empty()
                || device_path.is_empty()
        }
        UiCommandPayload::RemoveInputBinding { path, .. } => path.is_empty(),
        UiCommandPayload::SetInputBindingDevicePath {
            path, device_path, ..
        } => path.is_empty() || device_path.is_empty(),
        UiCommandPayload::SetInputBindingProcessorByIndex { path, .. } => path.is_empty(),
        UiCommandPayload::RemoveInputBindingById { path, binding_id }
        | UiCommandPayload::SelectInputBinding { path, binding_id }
        | UiCommandPayload::SetInputBindingTrigger {
            path, binding_id, ..
        }
        | UiCommandPayload::SetInputBindingProcessor {
            path, binding_id, ..
        }
        | UiCommandPayload::BeginInputBindingCapture { path, binding_id } => {
            path.is_empty() || binding_id.is_empty()
        }
        UiCommandPayload::SetInputBindingDevicePathById {
            path,
            binding_id,
            device_path,
        } => path.is_empty() || binding_id.is_empty() || device_path.is_empty(),
        UiCommandPayload::CommitCapturedInputBinding {
            path,
            binding_id,
            device_path,
        } => path.is_empty() || binding_id.is_empty() || device_path.is_empty(),
        UiCommandPayload::CancelInputBindingCapture { path } => path.is_empty(),
        UiCommandPayload::PreviewInputMapping { path, .. } => path.is_empty(),
        UiCommandPayload::SetInputMappingReportLevel { path, .. } => path.is_empty(),
        UiCommandPayload::CreateRuleAsset {
            path,
            rule_id,
            display_name: _,
            phase: _,
        } => path.is_empty() || rule_id.is_empty(),
        UiCommandPayload::SetRuleTrigger { path, trigger, .. } => {
            path.is_empty() || trigger.is_null()
        }
        UiCommandPayload::AddRuleStatement {
            path, statement, ..
        }
        | UiCommandPayload::UpdateRuleStatement {
            path, statement, ..
        } => path.is_empty() || statement.is_null(),
        UiCommandPayload::RemoveRuleStatement { path, .. } => path.is_empty(),
        UiCommandPayload::AddRuleOperation {
            path, operation, ..
        }
        | UiCommandPayload::UpdateRuleOperation {
            path, operation, ..
        } => path.is_empty() || operation.is_null(),
        UiCommandPayload::RemoveRuleOperation { path, .. } => path.is_empty(),
        UiCommandPayload::SelectRuleCard { path, card_id }
        | UiCommandPayload::RemoveRuleCard { path, card_id, .. } => {
            path.is_empty() || card_id.is_empty()
        }
        UiCommandPayload::SetRuleCardField {
            path,
            card_id,
            field_path,
            value,
            ..
        } => path.is_empty() || card_id.is_empty() || field_path.is_empty() || value.is_null(),
        UiCommandPayload::AddRuleCard {
            path,
            card_kind,
            value,
            ..
        } => path.is_empty() || card_kind.is_empty() || value.is_null(),
        UiCommandPayload::SelectRuleGraphNode { path, node_id } => {
            path.is_empty() || node_id.is_empty()
        }
        UiCommandPayload::SelectTraceEntry { entry_id } => entry_id.is_empty(),
        UiCommandPayload::SelectReportEntry { report_id }
        | UiCommandPayload::CopyReportAiContext { report_id }
        | UiCommandPayload::OpenRawReport { report_id }
        | UiCommandPayload::RevealReportPath { report_id } => report_id.is_empty(),
        UiCommandPayload::OpenRelatedReportArtifact {
            report_id,
            artifact_id,
        } => report_id.is_empty() || artifact_id.is_empty(),
        UiCommandPayload::AiSubmitPrompt { prompt }
        | UiCommandPayload::GenerateProjectPatchFromPrompt { prompt } => prompt.is_empty(),
        UiCommandPayload::ImportProjectPatch {
            source_label,
            raw_json,
            file_path,
            ..
        }
        | UiCommandPayload::PreviewImportedProjectPatch {
            source_label,
            raw_json,
            file_path,
            ..
        } => {
            source_label.is_empty()
                || raw_json.as_deref().is_none_or(str::is_empty)
                    && file_path.as_deref().is_none_or(str::is_empty)
        }
        UiCommandPayload::ApplyImportedProjectPatch { proposal_id } => proposal_id.is_empty(),
        UiCommandPayload::ParkProjectWorkItem { work_item_id }
        | UiCommandPayload::ResumeProjectWorkItem { work_item_id }
        | UiCommandPayload::ReopenProjectWorkItem { work_item_id } => work_item_id.is_empty(),
        UiCommandPayload::ApproveProjectChange { proposal_digest } => proposal_digest.is_empty(),
        UiCommandPayload::AdvanceProjectProduction { run_id }
        | UiCommandPayload::CancelProjectProduction { run_id }
        | UiCommandPayload::RecoverProjectProduction { run_id } => run_id.is_empty(),
        UiCommandPayload::ApproveGatewayAccessRequest { request_id }
        | UiCommandPayload::RejectGatewayAccessRequest { request_id } => request_id.is_empty(),
        UiCommandPayload::SetGatewayAccessPage { .. } => false,
        UiCommandPayload::AiAcceptProposedCommand { proposal_id }
        | UiCommandPayload::AiRejectProposedCommand { proposal_id } => proposal_id.is_empty(),
        UiCommandPayload::CreateProject { path, name } => path.is_empty() || name.is_empty(),
        UiCommandPayload::StartCreateProjectWithAi { draft_path } => draft_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty()),
        UiCommandPayload::SelectRecentProject { path } => path.is_empty(),
        UiCommandPayload::BeginAssetPick { field_id }
        | UiCommandPayload::DropAssetOnInspectorField { field_id, .. } => field_id.is_empty(),
        UiCommandPayload::RefreshRecentProjects
        | UiCommandPayload::SelectAssetBrowserEntry { .. }
        | UiCommandPayload::OpenAssetBrowserEntry { .. }
        | UiCommandPayload::SetAssetBrowserFolder { .. }
        | UiCommandPayload::SetAssetBrowserSearch { .. }
        | UiCommandPayload::SetAiPromptDraft { .. }
        | UiCommandPayload::CancelLlmPatchRequest
        | UiCommandPayload::SetAssetBrowserKindFilter { .. }
        | UiCommandPayload::AssetBrowserToolbar { .. }
        | UiCommandPayload::ScrollAssetBrowser { .. }
        | UiCommandPayload::ConfirmAssetPick
        | UiCommandPayload::CancelAssetPick
        | UiCommandPayload::SetWorkspaceViewMode { .. }
        | UiCommandPayload::SetAuthoringWorkflowStep { .. }
        | UiCommandPayload::ReloadRuntimePackage
        | UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring
        | UiCommandPayload::UndoSceneEdit
        | UiCommandPayload::RedoSceneEdit
        | UiCommandPayload::TickOneFrame
        | UiCommandPayload::PickRuntimeEntityAt { .. }
        | UiCommandPayload::Play
        | UiCommandPayload::Pause
        | UiCommandPayload::StepFrame
        | UiCommandPayload::StopPlaySession
        | UiCommandPayload::SetGameViewTarget { .. }
        | UiCommandPayload::SetGameViewMaximizeOnPlay { .. }
        | UiCommandPayload::ToggleGameViewMaximizeOnPlay
        | UiCommandPayload::ResetRuntime
        | UiCommandPayload::ExportDesktopPackage { .. }
        | UiCommandPayload::BuildAndRunDesktopPackage { .. }
        | UiCommandPayload::BuildReleasePackage { .. }
        | UiCommandPayload::SaveReleaseProfile
        | UiCommandPayload::SetReleaseProfileIcon { .. }
        | UiCommandPayload::OpenBuildOutput
        | UiCommandPayload::OpenBuildReport
        | UiCommandPayload::ClearConsole
        | UiCommandPayload::ApproveProjectRuntimeTrust { .. }
        | UiCommandPayload::DenyProjectRuntimeTrust { .. }
        | UiCommandPayload::CancelProjectRuntimeTrust { .. }
        | UiCommandPayload::RefreshReports => false,
    }
}

fn payload_kind(payload: &UiCommandPayload) -> &'static str {
    editor_ui_model::workspace_payload_kind(payload)
}

fn context_next_action(requirement: &ManualAuthoringOperationRequirement) -> String {
    if requirement
        .required_context
        .contains(&ManualWalkthroughRequiredContext::OpenProject)
    {
        "open_project".to_string()
    } else if requirement
        .required_context
        .contains(&ManualWalkthroughRequiredContext::OpenSceneDocument)
    {
        "open_scene_document".to_string()
    } else {
        format!("select_context_for_{}", requirement.operation_id)
    }
}

fn suggest_next_action(domain: WorkspaceDomainKind) -> &'static str {
    match domain {
        WorkspaceDomainKind::Project => "project_authoring_productization",
        WorkspaceDomainKind::Scene => "scene_authoring_productization",
        WorkspaceDomainKind::Asset => "asset_import_productization",
        WorkspaceDomainKind::Prefab => "prefab_authoring_productization",
        WorkspaceDomainKind::Rule => "rule_authoring_productization",
        WorkspaceDomainKind::Aui => "aui_authoring_productization",
        WorkspaceDomainKind::Input => "input_mapping_authoring_productization",
        WorkspaceDomainKind::Play => "play_runtime_package_productization",
        WorkspaceDomainKind::Build => "build_export_productization",
        WorkspaceDomainKind::Report => "unified_report_panel_productization",
    }
}
