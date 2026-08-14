use crate::{
    CommandResult, CommandStatus, PropertyEditBuffer, PropertyEditCommitReport,
    PropertyEditDiagnostic, PropertyEditTarget, PropertyPath, PropertyTree, PropertyTreeSummary,
};
use editor_ui_model::{
    ui_command_id_for_payload, DiagnosticSeverity, EditorDiagnostic, EditorUiModel, UiCommand,
    UiCommandPayload, UiCommandSource,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSelection {
    pub selected_entity_ids: Vec<String>,
    pub primary_entity_id: Option<String>,
    pub selected_asset_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDiagnosticsSummary {
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub last_code: Option<String>,
}

impl WorkspaceDiagnosticsSummary {
    pub fn from_diagnostics(diagnostics: &[EditorDiagnostic]) -> Self {
        let mut summary = Self::default();
        for diagnostic in diagnostics {
            match diagnostic.severity {
                DiagnosticSeverity::Info => summary.info_count += 1,
                DiagnosticSeverity::Warning => summary.warning_count += 1,
                DiagnosticSeverity::Error => summary.error_count += 1,
            }
            summary.last_code = Some(diagnostic.code.clone());
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub active_scene_id: Option<String>,
    pub active_panel_id: Option<String>,
    pub focused_panel_id: Option<String>,
    pub hovered_panel_id: Option<String>,
    pub selection: WorkspaceSelection,
    pub selected_asset_ref: Option<String>,
    pub active_tool: String,
    pub dirty: Option<bool>,
    pub last_command_id: Option<String>,
    pub last_command_status: Option<CommandStatus>,
    pub diagnostics_summary: WorkspaceDiagnosticsSummary,
    pub property_editing: PropertyEditingWorkspaceState,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active_scene_id: None,
            active_panel_id: None,
            focused_panel_id: None,
            hovered_panel_id: None,
            selection: WorkspaceSelection::default(),
            selected_asset_ref: None,
            active_tool: "select".to_string(),
            dirty: None,
            last_command_id: None,
            last_command_status: None,
            diagnostics_summary: WorkspaceDiagnosticsSummary::default(),
            property_editing: PropertyEditingWorkspaceState::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyEditingWorkspaceState {
    pub tree_summary: PropertyTreeSummary,
    pub focused_property_path: Option<String>,
    pub editing: bool,
    pub dirty: bool,
    pub last_commit_status: Option<PropertyEditingCommitStatus>,
    pub last_diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyEditingCommitStatus {
    Committed,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub scene_summary: Option<String>,
    pub authoring_workflow_summary: Option<String>,
    pub hierarchy_summary: String,
    pub selection_summary: String,
    pub inspector_summary: String,
    pub project_asset_summary: String,
    pub console_summary: String,
    pub domain_summaries: Vec<String>,
    pub dirty_summary: String,
    pub build_summary: Option<String>,
    pub play_summary: Option<String>,
    pub diagnostics_summary: WorkspaceDiagnosticsSummary,
    pub allowed_command_schema: Vec<String>,
}

impl WorkspaceContext {
    pub fn from_model(model: &EditorUiModel) -> Self {
        let entity_count = count_hierarchy_entities(&model.hierarchy.roots);
        let selected = model
            .hierarchy
            .selected_entity_id
            .clone()
            .or_else(|| model.inspector.selected_entity_id.clone());
        let inspector_field_count = model
            .inspector
            .sections
            .iter()
            .map(|section| section.fields.len())
            .sum::<usize>();
        let error_count = model.console.unread_error_count;
        let warning_count = model.console.unread_warning_count;

        Self {
            scene_summary: model
                .hierarchy
                .scene_id
                .clone()
                .map(|scene_id| format!("scene_id={scene_id} entity_count={entity_count}")),
            authoring_workflow_summary: Some(format!(
                "active_step={} {} missing_required={} recommended_tasks={}",
                model.authoring_workflow.active_step.as_str(),
                model.authoring_workflow.ai_context.summary,
                model
                    .authoring_workflow
                    .ai_context
                    .missing_required_items
                    .join(","),
                model.authoring_workflow.recommended_tasks.len()
            )),
            hierarchy_summary: format!(
                "root_count={} entity_count={entity_count}",
                model.hierarchy.roots.len()
            ),
            selection_summary: selected
                .as_ref()
                .map_or_else(|| "no_selection".to_string(), |id| format!("entity={id}")),
            inspector_summary: format!(
                "title={} section_count={} field_count={inspector_field_count} readonly={}",
                model.inspector.title,
                model.inspector.sections.len(),
                model.inspector.readonly
            ),
            project_asset_summary: model.project_browser.selected_path.as_ref().map_or_else(
                || "project_browser_selection=none".to_string(),
                |path| format!("project_browser_selection={path}"),
            ),
            console_summary: format!(
                "entries={} warnings={warning_count} errors={error_count}",
                model.console.entries.len()
            ),
            domain_summaries: model
                .project_authoring_workspace
                .domains
                .iter()
                .map(|domain| {
                    format!(
                        "{} status={:?} items={} dirty={} summary={}",
                        domain.kind.as_str(),
                        domain.status,
                        domain.item_count,
                        domain.dirty,
                        domain.summary
                    )
                })
                .collect(),
            dirty_summary: if model.project_authoring_workspace.dirty_domains.is_empty() {
                "dirty_domains=none".to_string()
            } else {
                format!(
                    "dirty_domains={}",
                    model
                        .project_authoring_workspace
                        .dirty_domains
                        .iter()
                        .map(|domain| domain.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            },
            build_summary: model
                .project_authoring_workspace
                .report
                .build_status
                .clone(),
            play_summary: model.project_authoring_workspace.report.play_status.clone(),
            diagnostics_summary: WorkspaceDiagnosticsSummary::from_diagnostics(&model.diagnostics),
            allowed_command_schema: model.ai_panel.allowed_command_ids.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCommandExecutionReport {
    pub command_id: String,
    pub source: UiCommandSource,
    pub normalized_command_id: String,
    pub status: CommandStatus,
    pub diagnostics_summary: WorkspaceDiagnosticsSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceReport {
    pub command_count: u64,
    pub last_command_id: Option<String>,
    pub last_command_status: Option<CommandStatus>,
    pub active_scene_id: Option<String>,
    pub primary_entity_id: Option<String>,
    pub selected_asset_ref: Option<String>,
    pub diagnostics_summary: WorkspaceDiagnosticsSummary,
    pub property_editing: PropertyEditingWorkspaceState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorAuthoringWorkspace {
    state: WorkspaceState,
    context: WorkspaceContext,
    property_tree: PropertyTree,
    property_edit_buffer: PropertyEditBuffer,
    command_count: u64,
    last_execution: Option<WorkspaceCommandExecutionReport>,
}

impl Default for EditorAuthoringWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorAuthoringWorkspace {
    pub fn new() -> Self {
        Self {
            state: WorkspaceState::default(),
            context: WorkspaceContext::default(),
            property_tree: PropertyTree::default(),
            property_edit_buffer: PropertyEditBuffer::new(),
            command_count: 0,
            last_execution: None,
        }
    }

    pub fn refresh_from_model(&mut self, model: &EditorUiModel) {
        self.state.active_scene_id = model.hierarchy.scene_id.clone();
        self.state.selection.primary_entity_id = model
            .hierarchy
            .selected_entity_id
            .clone()
            .or_else(|| model.inspector.selected_entity_id.clone());
        self.state.selection.selected_entity_ids = self
            .state
            .selection
            .primary_entity_id
            .iter()
            .cloned()
            .collect();
        self.state.selected_asset_ref = model.project_browser.selected_path.clone();
        self.state.diagnostics_summary =
            WorkspaceDiagnosticsSummary::from_diagnostics(&model.diagnostics);
        self.context = WorkspaceContext::from_model(model);
        self.property_tree = PropertyTree::from_inspector_model(&model.inspector);
        self.state.property_editing.tree_summary = self.property_tree.summary();
    }

    pub fn set_panel_state(
        &mut self,
        active_panel_id: Option<String>,
        focused_panel_id: Option<String>,
        hovered_panel_id: Option<String>,
    ) {
        self.state.active_panel_id = active_panel_id;
        self.state.focused_panel_id = focused_panel_id;
        self.state.hovered_panel_id = hovered_panel_id;
    }

    pub fn normalize_command(&mut self, command: UiCommand) -> UiCommand {
        self.command_count += 1;
        match (&command.source, &command.payload) {
            (UiCommandSource::Hierarchy, UiCommandPayload::SelectEntity { entity_id }) => {
                UiCommand {
                    command_id: workspace_command_id_for_payload(
                        &UiCommandPayload::SelectSceneEntity {
                            entity_id: entity_id.clone(),
                        },
                    )
                    .to_string(),
                    source: UiCommandSource::Hierarchy,
                    request_id: command.request_id,
                    payload: UiCommandPayload::SelectSceneEntity {
                        entity_id: entity_id.clone(),
                    },
                }
            }
            _ => UiCommand {
                command_id: workspace_command_id_for_payload(&command.payload).to_string(),
                ..command
            },
        }
    }

    pub fn record_command_result(&mut self, command: &UiCommand, result: &CommandResult) {
        self.state.last_command_id = Some(result.command_id.clone());
        self.state.last_command_status = Some(result.status);
        self.state.diagnostics_summary =
            WorkspaceDiagnosticsSummary::from_diagnostics(&result.diagnostics);
        if let UiCommandPayload::PlaceAssetIntoScene {
            asset_id,
            asset_guid,
            ..
        } = &command.payload
        {
            self.state.selected_asset_ref = asset_guid.clone().or_else(|| Some(asset_id.clone()));
        }
        self.last_execution = Some(WorkspaceCommandExecutionReport {
            command_id: command.command_id.clone(),
            source: command.source.clone(),
            normalized_command_id: result.command_id.clone(),
            status: result.status,
            diagnostics_summary: self.state.diagnostics_summary.clone(),
        });
    }

    pub fn begin_property_edit(
        &mut self,
        path: impl AsRef<str>,
    ) -> Result<(), PropertyEditDiagnostic> {
        let path = PropertyPath::parse(path.as_ref())?;
        let node = self.property_tree.find(&path).ok_or_else(|| {
            PropertyEditDiagnostic::error(
                "workspace.property.node_missing",
                format!("Cannot focus missing property: {path}"),
            )
        })?;
        if node.metadata.readonly || !node.metadata.editable {
            return Err(PropertyEditDiagnostic::error(
                "workspace.property.readonly",
                format!("Cannot edit readonly property: {path}"),
            )
            .with_path(path.to_string()));
        }
        self.property_edit_buffer.begin_edit(node);
        self.state.property_editing.focused_property_path = Some(path.to_string());
        self.state.property_editing.editing = true;
        self.state.property_editing.dirty = false;
        self.state.property_editing.last_commit_status = None;
        self.state.property_editing.last_diagnostic_code = None;
        Ok(())
    }

    pub fn input_property_text(&mut self, text: &str) {
        self.property_edit_buffer.input_text(text);
        self.state.property_editing.dirty = self.property_edit_buffer.dirty;
    }

    pub fn replace_property_text(&mut self, text: impl Into<String>) {
        self.property_edit_buffer.replace_text(text);
        self.state.property_editing.dirty = self.property_edit_buffer.dirty;
    }

    pub fn update_property_composition(&mut self, preedit_text: impl Into<String>) {
        self.property_edit_buffer.update_composition(preedit_text);
        self.state.property_editing.editing = true;
    }

    pub fn commit_property_composition(&mut self, committed_text: impl Into<String>) {
        self.property_edit_buffer.commit_composition(committed_text);
        self.state.property_editing.dirty = self.property_edit_buffer.dirty;
    }

    pub fn commit_property_edit(
        &mut self,
        request_id: impl Into<String>,
    ) -> Result<(PropertyEditCommitReport, UiCommand), PropertyEditDiagnostic> {
        let report = self.property_edit_buffer.commit(&self.property_tree)?;
        let command = report.command.as_ref().ok_or_else(|| {
            PropertyEditDiagnostic::error(
                "workspace.property.command_missing",
                "Property edit commit did not produce command.",
            )
        })?;
        let ui_command = command.to_ui_command(request_id)?;
        self.state.property_editing.last_commit_status =
            Some(PropertyEditingCommitStatus::Committed);
        self.state.property_editing.dirty = false;
        Ok((report, ui_command))
    }

    pub fn cancel_property_edit(&mut self) -> PropertyEditCommitReport {
        let report = self.property_edit_buffer.cancel();
        self.state.property_editing.focused_property_path = None;
        self.state.property_editing.editing = false;
        self.state.property_editing.dirty = false;
        self.state.property_editing.last_commit_status =
            Some(PropertyEditingCommitStatus::Cancelled);
        report
    }

    pub fn property_tree(&self) -> &PropertyTree {
        &self.property_tree
    }

    pub fn property_edit_target_for_path(
        &self,
        path: impl AsRef<str>,
    ) -> Result<PropertyEditTarget, PropertyEditDiagnostic> {
        let path = PropertyPath::parse(path.as_ref())?;
        let node = self.property_tree.find(&path).ok_or_else(|| {
            PropertyEditDiagnostic::error(
                "workspace.property.node_missing",
                format!("Cannot resolve missing property: {path}"),
            )
        })?;
        Ok(PropertyEditTarget {
            entity_id: self.property_tree.selected_entity_id.clone(),
            persistence: self.property_tree.persistence,
            path,
            component_type: node.metadata.component_type.clone(),
            field_path: node.metadata.field_path.clone(),
        })
    }

    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    pub fn context(&self) -> &WorkspaceContext {
        &self.context
    }

    pub fn last_execution(&self) -> Option<&WorkspaceCommandExecutionReport> {
        self.last_execution.as_ref()
    }

    pub fn report(&self) -> WorkspaceReport {
        WorkspaceReport {
            command_count: self.command_count,
            last_command_id: self.state.last_command_id.clone(),
            last_command_status: self.state.last_command_status,
            active_scene_id: self.state.active_scene_id.clone(),
            primary_entity_id: self.state.selection.primary_entity_id.clone(),
            selected_asset_ref: self.state.selected_asset_ref.clone(),
            diagnostics_summary: self.state.diagnostics_summary.clone(),
            property_editing: self.state.property_editing.clone(),
        }
    }
}

pub fn workspace_command_id_for_payload(payload: &UiCommandPayload) -> &'static str {
    ui_command_id_for_payload(payload)
}

fn count_hierarchy_entities(nodes: &[editor_ui_model::HierarchyNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_hierarchy_entities(&node.children))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_for_test;
    use editor_ui_model::{
        AiPanelModel, AssetPlacementMode, BuildExportModel, ConsoleModel, EditorUiMode,
        EditorUiModel, HierarchyModel, HierarchyNode, InputMappingAuthoringModel, InspectorField,
        InspectorModel, InspectorSection, InspectorValue, InspectorValueType, PanelLayoutModel,
        ProjectAuthoringWorkspaceModel, ProjectBrowserModel, ProjectLauncherModel, RuntimeRunState,
        RuntimeTraceModel, ToolbarModel, UiCommandPayload, Vec3, ViewportModel, WorkspaceViewMode,
    };

    fn empty_model() -> EditorUiModel {
        EditorUiModel {
            revision: 1,
            frame: 0,
            mode: EditorUiMode::AuthoringWorkspace,
            project_launcher: ProjectLauncherModel::empty(),
            project_intent: editor_ui_model::ProjectIntentWorkspaceModel::empty(),
            project_browser: ProjectBrowserModel::empty(),
            asset_browser: editor_ui_model::AssetBrowserModel::empty(),
            build_export: BuildExportModel::empty(),
            report_panel: editor_ui_model::ReportPanelModel::empty(),
            input_mapping_authoring: InputMappingAuthoringModel::empty(),
            rule_authoring: editor_ui_model::RuleAuthoringModel::empty(),
            animator2d_authoring: editor_ui_model::Animator2DAuthoringModel::default(),
            project_authoring_workspace: ProjectAuthoringWorkspaceModel::empty(),
            authoring_workflow: editor_ui_model::AuthoringWorkflowModel::empty(),
            workspace_view_mode: WorkspaceViewMode::SceneView,
            active_runtime_package: None,
            panels: PanelLayoutModel::fixed_mvp(),
            toolbar: ToolbarModel {
                commands: Vec::new(),
                runtime_state: RuntimeRunState::NoPackage,
                game_view_layout: editor_ui_model::GameViewLayoutState::default(),
            },
            hierarchy: HierarchyModel {
                scene_id: None,
                roots: Vec::new(),
                selected_entity_id: None,
                authoring_view: editor_ui_model::HierarchyAuthoringView::EntityTree,
                visual_order: None,
                source_domain: editor_ui_model::HierarchySourceDomain::Empty,
                status: "empty".to_string(),
            },
            inspector: InspectorModel {
                selected_entity_id: None,
                title: "No Selection".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: editor_ui_model::InspectorPersistence::ReadOnly,
            },
            viewport: ViewportModel {
                scene_id: None,
                frame: 0,
                frame_hash: None,
                texture_id: None,
                target_id: None,
                renderable_count: 0,
                selected_entity: None,
                renderables: Vec::new(),
                collider_overlay: editor_ui_model::ColliderOverlayModel::default(),
            },
            console: ConsoleModel {
                entries: Vec::new(),
                unread_error_count: 0,
                unread_warning_count: 0,
            },
            runtime_trace: RuntimeTraceModel {
                frame: 0,
                entries: Vec::new(),
                selected_entry_id: None,
            },
            ai_panel: AiPanelModel {
                prompt_placeholder: "Describe an editor change...".to_string(),
                prompt_draft: String::new(),
                messages: Vec::new(),
                gateway_access: Default::default(),
                proposed_commands: Vec::new(),
                allowed_command_ids: Vec::new(),
                busy: false,
                stage: editor_ui_model::AiPanelStage::Idle,
                status_summary: None,
            },
            project_runtime_trust_prompt: None,
            interaction_feedback: None,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn editor_authoring_workspace_builds_context_from_ui_model() {
        let mut model = empty_model();
        model.hierarchy.scene_id = Some("scene-main".to_string());
        model.hierarchy.roots = vec![HierarchyNode {
            entity_id: "entity-player".to_string(),
            label: "Player".to_string(),
            alive: true,
            children: Vec::new(),
        }];
        model.hierarchy.selected_entity_id = Some("entity-player".to_string());
        model.inspector.selected_entity_id = Some("entity-player".to_string());

        let mut workspace = EditorAuthoringWorkspace::new();
        workspace.refresh_from_model(&model);

        assert_eq!(
            workspace.state().active_scene_id.as_deref(),
            Some("scene-main")
        );
        assert_eq!(
            workspace.state().selection.primary_entity_id.as_deref(),
            Some("entity-player")
        );
        assert!(workspace
            .context()
            .hierarchy_summary
            .contains("entity_count=1"));
        assert!(workspace
            .context()
            .selection_summary
            .contains("entity-player"));
        assert!(workspace
            .context()
            .authoring_workflow_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("active_step=project")));
    }

    #[test]
    fn editor_authoring_workspace_context_includes_project_authoring_domains() {
        let mut model = empty_model();
        model.project_authoring_workspace = ProjectAuthoringWorkspaceModel {
            project_root: Some("D:/Projects/PlaneGame".to_string()),
            project_id: Some("project-plane".to_string()),
            active_scene_id: Some("scene-main".to_string()),
            active_document: None,
            selection: editor_ui_model::WorkspaceSelectionSummary::default(),
            domains: vec![editor_ui_model::WorkspaceDomainSummary::new(
                editor_ui_model::WorkspaceDomainKind::Build,
                "Build",
                editor_ui_model::WorkspaceDomainStatus::Ready,
                "profile=dev status=success diagnostics=0",
            )],
            dirty_domains: vec![editor_ui_model::WorkspaceDomainKind::Scene],
            diagnostics: editor_ui_model::WorkspaceDiagnosticsSummary::default(),
            empty_message: String::new(),
            report: editor_ui_model::WorkspaceReportSummary {
                project_status: "open".to_string(),
                dirty_domains: vec![editor_ui_model::WorkspaceDomainKind::Scene],
                diagnostics: editor_ui_model::WorkspaceDiagnosticsSummary::default(),
                report_count: 0,
                evidence_count: 0,
                next_action_count: 0,
                last_command: None,
                last_transaction: None,
                build_status: Some("success".to_string()),
                play_status: Some("stopped".to_string()),
            },
        };

        let mut workspace = EditorAuthoringWorkspace::new();
        workspace.refresh_from_model(&model);

        assert!(workspace
            .context()
            .domain_summaries
            .iter()
            .any(|summary| summary.contains("build status=Ready")));
        assert_eq!(workspace.context().dirty_summary, "dirty_domains=scene");
        assert_eq!(
            workspace.context().build_summary.as_deref(),
            Some("success")
        );
    }

    #[test]
    fn editor_authoring_workspace_normalizes_hierarchy_select_entity() {
        let mut workspace = EditorAuthoringWorkspace::new();
        let command = UiCommand {
            command_id: "select_entity".to_string(),
            source: UiCommandSource::Hierarchy,
            request_id: "request-1".to_string(),
            payload: UiCommandPayload::SelectEntity {
                entity_id: "entity-a".to_string(),
            },
        };

        let normalized = workspace.normalize_command(command);

        assert_eq!(normalized.command_id, "select_scene_entity");
        assert!(matches!(
            normalized.payload,
            UiCommandPayload::SelectSceneEntity { .. }
        ));
        assert_eq!(workspace.report().command_count, 1);
    }

    #[test]
    fn editor_authoring_workspace_records_place_asset_result() {
        let mut workspace = EditorAuthoringWorkspace::new();
        let command = command_for_test(UiCommandPayload::PlaceAssetIntoScene {
            asset_id: "asset-ship".to_string(),
            asset_type: "texture2d".to_string(),
            asset_guid: Some("guid-ship".to_string()),
            target_parent_id: None,
            local_position: Some(Vec3 {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            }),
            placement_mode: AssetPlacementMode::WorldOrigin,
        });
        let result = CommandResult {
            transaction_id: "tx-1".to_string(),
            request_id: command.request_id.clone(),
            command_id: command.command_id.clone(),
            status: CommandStatus::Committed,
            diagnostics: Vec::new(),
            console_entries: Vec::new(),
            state_changes: Vec::new(),
            ui_model_revision: 1,
        };

        workspace.record_command_result(&command, &result);

        let report = workspace.report();
        assert_eq!(
            report.last_command_id.as_deref(),
            Some("place_asset_into_scene")
        );
        assert_eq!(report.last_command_status, Some(CommandStatus::Committed));
        assert_eq!(report.selected_asset_ref.as_deref(), Some("guid-ship"));
    }

    #[test]
    fn editor_authoring_workspace_builds_property_tree_summary() {
        let mut model = empty_model();
        model.inspector = InspectorModel {
            selected_entity_id: Some("entity-player".to_string()),
            title: "Player".to_string(),
            readonly: false,
            persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
            sections: vec![InspectorSection {
                section_id: "transform".to_string(),
                title: "Transform".to_string(),
                fields: vec![InspectorField {
                    field_id: "transform.localPosition".to_string(),
                    label: "localPosition".to_string(),
                    value: InspectorValue::Vec3(Vec3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    }),
                    value_type: InspectorValueType::Vec3,
                    path: "transform.localPosition".to_string(),
                    readonly: false,
                    editable: true,
                }],
            }],
        };

        let mut workspace = EditorAuthoringWorkspace::new();
        workspace.refresh_from_model(&model);

        assert_eq!(workspace.property_tree().summary().property_count, 1);
        assert_eq!(
            workspace
                .report()
                .property_editing
                .tree_summary
                .editable_count,
            1
        );
        assert!(workspace
            .report()
            .property_editing
            .tree_summary
            .editable_paths
            .contains(&"transform.localPosition".to_string()));
    }

    #[test]
    fn editor_authoring_workspace_commits_property_edit_to_ui_command() {
        let mut model = empty_model();
        model.inspector = InspectorModel {
            selected_entity_id: Some("entity-player".to_string()),
            title: "Player".to_string(),
            readonly: false,
            persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
            sections: vec![InspectorSection {
                section_id: "transform".to_string(),
                title: "Transform".to_string(),
                fields: vec![InspectorField {
                    field_id: "transform.localPosition".to_string(),
                    label: "localPosition".to_string(),
                    value: InspectorValue::Vec3(Vec3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    }),
                    value_type: InspectorValueType::Vec3,
                    path: "transform.localPosition".to_string(),
                    readonly: false,
                    editable: true,
                }],
            }],
        };
        let mut workspace = EditorAuthoringWorkspace::new();
        workspace.refresh_from_model(&model);
        workspace
            .begin_property_edit("transform.localPosition")
            .unwrap();
        workspace.replace_property_text("4,5,6");

        let (_report, command) = workspace
            .commit_property_edit("request-property-1")
            .unwrap();

        assert_eq!(command.command_id, "set_scene_transform");
        assert!(matches!(
            command.payload,
            UiCommandPayload::SetSceneTransform { .. }
        ));
        assert_eq!(
            workspace.report().property_editing.last_commit_status,
            Some(PropertyEditingCommitStatus::Committed)
        );
    }
}
