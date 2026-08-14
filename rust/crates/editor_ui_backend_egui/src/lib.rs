use editor_ui_model::{EditorUiModel, UiCommand, UiCommandPayload, UiCommandSource};

pub struct EguiBackendSummary {
    pub panel_count: usize,
    pub command_count: usize,
    pub renderable_count: usize,
}

pub fn summarize_model_for_egui_backend(model: &EditorUiModel) -> EguiBackendSummary {
    EguiBackendSummary {
        panel_count: model
            .panels
            .regions
            .iter()
            .map(|region| region.panel_ids.len())
            .sum(),
        command_count: model.toolbar.commands.len(),
        renderable_count: model.viewport.renderable_count,
    }
}

pub fn create_tick_command_from_toolbar(request_id: impl Into<String>) -> UiCommand {
    UiCommand {
        command_id: "tick_one_frame".to_string(),
        source: UiCommandSource::Toolbar,
        request_id: request_id.into(),
        payload: UiCommandPayload::TickOneFrame,
    }
}

pub struct HeadlessEguiBackendContext {
    pub backend_name: &'static str,
}

pub fn egui_context_smoke() -> HeadlessEguiBackendContext {
    HeadlessEguiBackendContext {
        backend_name: "editor_ui_backend_egui.headless",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_model::{
        AiPanelModel, AuthoringWorkflowModel, BuildExportModel, ConsoleModel, EditorUiMode,
        HierarchyModel, InputMappingAuthoringModel, InspectorModel, PanelLayoutModel,
        ProjectAuthoringWorkspaceModel, ProjectBrowserModel, ProjectLauncherModel, RuntimeRunState,
        RuntimeTraceModel, ToolbarCommand, ToolbarModel, ViewportModel, WorkspaceViewMode,
    };

    #[test]
    fn egui_backend_summarizes_ui_model_without_business_mutation() {
        let model = EditorUiModel {
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
            authoring_workflow: AuthoringWorkflowModel::empty(),
            project_authoring_workspace: ProjectAuthoringWorkspaceModel::empty(),
            workspace_view_mode: WorkspaceViewMode::SceneView,
            active_runtime_package: None,
            panels: PanelLayoutModel::fixed_mvp(),
            toolbar: ToolbarModel {
                commands: vec![ToolbarCommand {
                    command_id: "tick_one_frame".to_string(),
                    label: "Tick".to_string(),
                    enabled: true,
                    reason_disabled: None,
                }],
                runtime_state: RuntimeRunState::Paused,
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
                renderable_count: 2,
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
        };
        let summary = summarize_model_for_egui_backend(&model);
        assert_eq!(summary.panel_count, 7);
        assert_eq!(summary.command_count, 1);
        assert_eq!(summary.renderable_count, 2);
    }

    #[test]
    fn egui_backend_only_emits_ui_command() {
        let command = create_tick_command_from_toolbar("request-1");
        assert_eq!(command.command_id, "tick_one_frame");
        assert_eq!(command.payload, UiCommandPayload::TickOneFrame);
    }

    #[test]
    fn egui_context_can_be_created_headlessly() {
        let context = egui_context_smoke();
        assert_eq!(context.backend_name, "editor_ui_backend_egui.headless");
    }
}
