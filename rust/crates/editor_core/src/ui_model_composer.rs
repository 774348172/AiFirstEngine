use editor_ui_model::{
    AiPanelModel, AssetBrowserModel, AssetQuery, AssetSelection, AuiSceneViewProjection,
    BuildExportCommand, BuildExportModel, BuildExportReportSummary, BuildProfileSummary,
    ColliderOverlayDiagnostic, ColliderOverlayItem, ColliderOverlayModel, ColliderOverlayShape,
    ConsoleLevel, ConsoleModel, DiagnosticSeverity, EditorDiagnostic, EditorGameViewScalePolicy,
    EditorGameViewTarget, EditorUiMode, EditorUiModel, EntitySummary, GameViewLayoutState,
    HierarchyAuthoringView, HierarchyModel, HierarchyNode, HierarchySourceDomain, InspectorField,
    InspectorModel, InspectorPersistence, InspectorSection, InspectorValue, InspectorValueType,
    PanelLayoutModel, ProjectAuthoringWorkspaceModel, ProjectChangeReviewModel, ProjectIntentModel,
    ProjectIntentWorkItemModel, ProjectIntentWorkspaceModel, ProjectLauncherCommand,
    ProjectLauncherModel, ProjectProductionModel, ReleaseBuildProfileModel,
    ReleasePackageReportSummary, RenderableSummary, ReportPanelModel, RuntimePackageSummary,
    RuntimeRunState, RuntimeTraceEntryView, RuntimeTraceModel, ToolbarCommand, ToolbarModel,
    TraceLevel, ViewportModel, WorkspaceDiagnosticsSummary as UiWorkspaceDiagnosticsSummary,
    WorkspaceDocumentSummary, WorkspaceDomainKind, WorkspaceDomainStatus, WorkspaceDomainSummary,
    WorkspaceReportSummary, WorkspaceSelectionSummary, WorkspaceSelectionTarget,
};
use engine_runtime::ids::EntityId;
use engine_runtime::physics2d::Shape2D;
use engine_runtime::render_snapshot::RenderSnapshot;
use engine_runtime::runtime_trace::RuntimeTrace;
use engine_runtime::world::World;
use std::collections::HashMap;
use std::path::Path;

use crate::services::project_service::{
    is_input_mapping_relative_path, normalize_project_relative_path,
};
use crate::services::scene_service::{editor_vec3_to_ui, runtime_vec3_to_ui};
use crate::{
    is_rule_asset_relative_path, scan_input_mapping_paths, AuiAuthoringService,
    AuiSceneAuthoringService, AuthoringWorkflowComposer, ColliderDebugDrawList, ColliderDebugShape,
    DesktopExportReport, DesktopExportStatus, EditorRuntimePlayState, EditorSession,
    EntitySelectionSource, InputMappingAuthoringService, InspectorContextAnchor,
    ManualWalkthroughCoverageAnalyzer, ManualWalkthroughCoverageInput, PlaySessionState,
    ProjectSession, ReportProviderContext, ReportRegistry, RuleAuthoringService,
};

impl EditorSession {
    pub fn inspector_context_anchor(&self) -> Option<InspectorContextAnchor> {
        if self.selected_aui_node.is_some() {
            return None;
        }
        if let Some(document) = &self.editor_scene_document {
            return self
                .scene_selection
                .primary_entity_id
                .as_ref()
                .filter(|entity_id| document.entity(entity_id).is_some())
                .map(|entity_id| InspectorContextAnchor::AuthoringEntity {
                    entity_id: entity_id.clone(),
                });
        }
        self.selected_entity_id
            .as_ref()
            .map(|entity_id| InspectorContextAnchor::RuntimeEntity {
                entity_id: entity_id.clone(),
                source: self
                    .selected_entity_source
                    .unwrap_or(EntitySelectionSource::OpenedRuntimePackage),
            })
    }

    pub fn build_inspector_model_for_anchor(
        &self,
        anchor: &InspectorContextAnchor,
    ) -> InspectorModel {
        let anchored_entity_id = match anchor {
            InspectorContextAnchor::AuthoringEntity { entity_id }
            | InspectorContextAnchor::RuntimeEntity { entity_id, .. } => entity_id,
        };
        let model = match anchor {
            InspectorContextAnchor::AuthoringEntity { entity_id } => {
                self.build_authoring_entity_inspector_model(entity_id)
            }
            InspectorContextAnchor::RuntimeEntity { entity_id, source } => {
                let world = if *source == EntitySelectionSource::ActiveGameViewRuntime {
                    self.editor_runtime_play_instance
                        .as_ref()
                        .map(|instance| instance.runtime_world())
                } else {
                    self.world.as_ref()
                };
                world.map(|world| {
                    self.build_runtime_entity_inspector_model(world, entity_id, *source)
                })
            }
        }
        .filter(|model| model.selected_entity_id.as_deref() == Some(anchored_entity_id.as_str()));
        model.unwrap_or_else(|| InspectorModel {
            selected_entity_id: Some(anchored_entity_id.clone()),
            title: "Locked target unavailable".to_string(),
            sections: Vec::new(),
            readonly: true,
            persistence: InspectorPersistence::ReadOnly,
        })
    }

    pub fn build_ui_model(&self) -> EditorUiModel {
        let frame = self
            .last_game_view_runtime_frame
            .as_ref()
            .map(|frame| frame.frame_index)
            .or_else(|| self.last_frame_output.as_ref().map(|output| output.frame))
            .unwrap_or(0);
        let mut project_authoring_workspace = self.build_project_authoring_workspace_model();
        let mut authoring_workflow =
            AuthoringWorkflowComposer::compose(&project_authoring_workspace);
        let manual_walkthrough_report =
            ManualWalkthroughCoverageAnalyzer::analyze(ManualWalkthroughCoverageInput {
                workspace: &project_authoring_workspace,
                workflow: &authoring_workflow,
                scenario_id: "editor-ui-model",
            });
        authoring_workflow.ai_context.manual_walkthrough_coverage =
            Some(manual_walkthrough_report.summary());
        if authoring_workflow
            .step(self.active_authoring_step)
            .is_some()
        {
            authoring_workflow.active_step = self.active_authoring_step;
            authoring_workflow.ai_context.active_step = self.active_authoring_step;
        }
        let report_panel = ReportRegistry::standard().build_model(
            &ReportProviderContext {
                session: self,
                workspace: &project_authoring_workspace,
                authoring_workflow: &authoring_workflow,
                manual_walkthrough_report: &manual_walkthrough_report,
            },
            self.selected_report_id.clone(),
        );
        apply_report_panel_to_workspace(&mut project_authoring_workspace, &report_panel);
        let asset_browser =
            self.build_asset_browser_model(self.asset_browser_state.ui_state.query.clone());
        EditorUiModel {
            revision: self.revision,
            frame,
            mode: if self.active_project_session.is_some() {
                EditorUiMode::AuthoringWorkspace
            } else {
                EditorUiMode::ProjectLauncher
            },
            project_launcher: self.build_project_launcher_model(),
            project_intent: self.build_project_intent_workspace_model(),
            project_browser: asset_browser.to_project_browser_model(),
            asset_browser,
            build_export: self.build_export_model(),
            report_panel,
            input_mapping_authoring: self.build_input_mapping_authoring_model(),
            rule_authoring: self.build_rule_authoring_model(),
            animator2d_authoring: self.animator2d_authoring_model(),
            project_authoring_workspace,
            authoring_workflow,
            workspace_view_mode: self.workspace_view_mode,
            active_runtime_package: self.runtime_package.as_ref().map(|package| {
                RuntimePackageSummary {
                    package_dir: package.package_dir.display().to_string(),
                    project_name: package.manifest.project.name.clone(),
                    project_version: package.manifest.project.version.clone(),
                    active_scene_id: package.manifest.active_scene_id.clone(),
                }
            }),
            panels: PanelLayoutModel::fixed_mvp(),
            toolbar: self.build_toolbar_model(),
            hierarchy: self.build_hierarchy_model(),
            inspector: self.build_inspector_model(),
            viewport: self.build_viewport_model(),
            console: self.build_console_model(),
            runtime_trace: self.build_runtime_trace_model(),
            ai_panel: self.build_ai_panel_model(),
            project_runtime_trust_prompt: None,
            interaction_feedback: None,
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn build_toolbar_model(&self) -> ToolbarModel {
        let has_package = self.runtime_package.is_some();
        let has_project = self.active_project_session.is_some();
        let has_play_source = has_package || has_project;
        let runtime_disabled_reason =
            (!has_package).then(|| "Open a Runtime Package first.".to_string());
        let project_runtime_blocker = self.project_runtime_play_blocker();
        let play_disabled_reason = project_runtime_blocker
            .as_ref()
            .map(|blocker| blocker.code.clone())
            .or_else(|| (!has_play_source).then(|| "Open or create a project first.".to_string()));
        let has_scene_document = self.editor_scene_document.is_some();
        let scene_disabled_reason =
            (!has_scene_document).then(|| "Open an editable Scene first.".to_string());
        let play_session_running = matches!(
            self.play_session_controller.state(),
            PlaySessionState::Preparing
                | PlaySessionState::Building
                | PlaySessionState::StagingPackage
                | PlaySessionState::Launching
                | PlaySessionState::Running
                | PlaySessionState::Stopping
        );
        let game_view_control_state = self
            .editor_runtime_play_instance
            .as_ref()
            .map(|instance| instance.control_state());
        let game_view_paused = matches!(
            game_view_control_state,
            Some(EditorRuntimePlayState::Paused)
        );
        let has_active_game_view = self.editor_runtime_play_instance.is_some();
        ToolbarModel {
            runtime_state: if game_view_paused {
                RuntimeRunState::Paused
            } else if play_session_running {
                RuntimeRunState::Playing
            } else if !has_play_source {
                RuntimeRunState::NoPackage
            } else {
                RuntimeRunState::Paused
            },
            game_view_layout: GameViewLayoutState {
                maximize_on_play: self.game_view_maximize_on_play,
                is_game_view_maximized: self.is_game_view_maximized,
                restore_workspace_region: self.game_view_restore_workspace_region.clone(),
                reason: self.game_view_maximize_reason.clone(),
                target: EditorGameViewTarget::new(
                    self.game_view_target.extent.width,
                    self.game_view_target.extent.height,
                    match self.game_view_target.scale_policy {
                        engine_runtime::game_view_presentation::GameViewScalePolicy::Contain => {
                            EditorGameViewScalePolicy::Contain
                        }
                        engine_runtime::game_view_presentation::GameViewScalePolicy::Stretch => {
                            EditorGameViewScalePolicy::Stretch
                        }
                    },
                ),
                target_editable: !play_session_running && !has_active_game_view,
            },
            commands: vec![
                toolbar_command(
                    "save_scene_document",
                    "Save",
                    has_scene_document,
                    scene_disabled_reason.clone(),
                ),
                toolbar_command(
                    "undo_scene_edit",
                    "Undo",
                    has_scene_document,
                    scene_disabled_reason.clone(),
                ),
                toolbar_command(
                    "redo_scene_edit",
                    "Redo",
                    has_scene_document,
                    scene_disabled_reason,
                ),
                toolbar_command("open_runtime_package", "Open Runtime Package", true, None),
                toolbar_command(
                    "reload_runtime_package",
                    "Reload",
                    has_package,
                    runtime_disabled_reason.clone(),
                ),
                toolbar_command(
                    "play",
                    "Play",
                    has_play_source
                        && project_runtime_blocker.is_none()
                        && (!play_session_running || game_view_paused),
                    play_disabled_reason,
                ),
                toolbar_command(
                    "pause",
                    "Pause",
                    has_active_game_view && !game_view_paused,
                    (!has_active_game_view)
                        .then(|| "Start Editor GameView Play first.".to_string()),
                ),
                toolbar_command(
                    "step_frame",
                    "Step Frame",
                    has_active_game_view && game_view_paused,
                    (!has_active_game_view)
                        .then(|| "Start Editor GameView Play first.".to_string())
                        .or_else(|| {
                            (!game_view_paused)
                                .then(|| "Pause Editor GameView Play before stepping.".to_string())
                        }),
                ),
                toolbar_command(
                    "stop_play_session",
                    "Stop",
                    play_session_running || has_active_game_view,
                    (!play_session_running && !has_active_game_view)
                        .then(|| "No active Play session.".to_string()),
                ),
                toolbar_command(
                    "toggle_game_view_maximize_on_play",
                    "Maximize on Play",
                    true,
                    None,
                ),
                toolbar_command(
                    "tick_one_frame",
                    "Tick 1 Frame",
                    has_package,
                    runtime_disabled_reason.clone(),
                ),
                toolbar_command(
                    "reset_runtime",
                    "Reset Runtime",
                    has_package,
                    runtime_disabled_reason,
                ),
            ],
        }
    }

    fn build_project_launcher_model(&self) -> ProjectLauncherModel {
        let mut model = ProjectLauncherModel::empty();
        model.selected_project_path = self.project_launcher.selected_project_path.clone();
        model.recent_projects = self.project_launcher.recent_projects.clone();
        model.commands = vec![
            ProjectLauncherCommand::new("open_project", "Open Project", true, None),
            ProjectLauncherCommand::new("create_project", "Create Project", true, None),
            ProjectLauncherCommand::new("create_with_ai", "Create with AI", true, None),
            ProjectLauncherCommand::new("refresh_recent_projects", "Refresh", true, None),
        ];
        model
    }

    fn build_project_intent_workspace_model(&self) -> ProjectIntentWorkspaceModel {
        let Ok(snapshot) = self.project_intent_snapshot() else {
            return ProjectIntentWorkspaceModel::empty();
        };
        let work_items = snapshot
            .work_item_summaries
            .iter()
            .map(|item| ProjectIntentWorkItemModel {
                work_item_id: item.work_item_id.clone(),
                kind: format!("{:?}", item.kind).to_ascii_lowercase(),
                title: item.title.clone(),
                status: format!("{:?}", item.status).to_ascii_lowercase(),
                ready: item.ready,
                revision: item.revision,
            })
            .collect::<Vec<_>>();
        let parked_count = snapshot
            .work_items
            .iter()
            .filter(|item| item.status == crate::WorkItemStatus::Parked)
            .count();
        let needs_evidence_count = snapshot
            .work_items
            .iter()
            .filter(|item| item.status == crate::WorkItemStatus::NeedsEvidence)
            .count();
        let active_count = snapshot
            .work_items
            .iter()
            .filter(|item| {
                !item.status.is_terminal() && item.status != crate::WorkItemStatus::Parked
            })
            .count();
        let latest_summary = snapshot
            .intent_events
            .last()
            .map(|event| event.sanitized_summary.clone());
        let intent = ProjectIntentModel {
            pre_project_draft_active: self.project_intent_workflow.storage_kind()
                == crate::ProjectIntentStorageKind::PreProjectDraft,
            journal_revision: snapshot.journal_revision,
            active_count,
            parked_count,
            needs_evidence_count,
            pending_normalization_count: snapshot.pending_normalization_event_ids.len(),
            work_items,
            latest_summary,
        };
        let change_review = snapshot
            .active_proposal
            .as_ref()
            .map(|proposal| ProjectChangeReviewModel {
                proposal_id: Some(proposal.proposal_id.clone()),
                proposal_digest: Some(proposal.proposal_digest.clone()),
                selected_work_item_count: proposal.selected_work_item_revisions.len(),
                user_visible_outcomes: proposal.user_visible_outcomes.clone(),
                explicit_exclusions: proposal.explicit_exclusions.clone(),
                risks: proposal.risks.clone(),
                required_decisions: proposal.required_decisions.clone(),
                approval_ready: proposal.required_decisions.is_empty()
                    && snapshot.active_approval.is_none(),
            })
            .unwrap_or_else(ProjectChangeReviewModel::empty);
        let production = snapshot
            .active_run
            .as_ref()
            .map(|run| ProjectProductionModel {
                run_id: Some(run.run_id.clone()),
                state: Some(format!("{:?}", run.state).to_ascii_lowercase()),
                active_step_id: run.active_step_id.clone(),
                completed_steps: run
                    .step_snapshots
                    .iter()
                    .filter(|step| step.state == crate::ProductionStepState::Applied)
                    .count(),
                total_steps: run.step_snapshots.len(),
                waiting_reason: run.decision_requests.first().cloned(),
                recovery_options: run.recovery_options.clone(),
                latest_result: run.preview_evidence.clone(),
            })
            .unwrap_or_else(ProjectProductionModel::empty);
        ProjectIntentWorkspaceModel {
            report_level: editor_ui_model::ProjectIntentReportLevel::Summary,
            intent,
            change_review,
            production,
        }
    }

    pub fn build_asset_browser_model(&self, query: AssetQuery) -> AssetBrowserModel {
        if self.active_project_session.is_none() {
            return AssetBrowserModel::empty();
        }

        let selection = if self
            .asset_browser_state
            .ui_state
            .selection
            .selected_entry_keys
            .is_empty()
            && self
                .asset_browser_state
                .ui_state
                .selection
                .selected_paths
                .is_empty()
        {
            self.selected_project_browser_path
                .as_ref()
                .map(|path| AssetSelection::single(path.clone(), None))
                .unwrap_or_default()
        } else {
            self.asset_browser_state.ui_state.selection.clone()
        };
        self.asset_browser_state.model(query, selection)
    }

    pub fn build_input_mapping_authoring_model(
        &self,
    ) -> editor_ui_model::InputMappingAuthoringModel {
        let Some(session) = &self.active_project_session else {
            return editor_ui_model::InputMappingAuthoringModel::empty();
        };
        let mapping_paths = scan_input_mapping_paths(&session.project_root);
        let selected_path = self
            .selected_project_browser_path
            .as_ref()
            .filter(|path| is_input_mapping_relative_path(path))
            .cloned()
            .or_else(|| mapping_paths.first().cloned());
        let editor_state = self.input_mapping_editor_state.as_ref().filter(|state| {
            selected_path
                .as_ref()
                .is_some_and(|path| path == &state.selected_path)
        });
        let loaded_mapping;
        let mapping = if let Some(state) = editor_state {
            Some(&state.draft_mapping)
        } else {
            loaded_mapping = selected_path.as_ref().and_then(|path| {
                InputMappingAuthoringService::load(&session.project_root, path).ok()
            });
            loaded_mapping.as_ref()
        };
        InputMappingAuthoringService::build_model_with_editor_state(
            Some(&session.project_root),
            selected_path,
            mapping,
            mapping_paths.len(),
            editor_state,
        )
    }

    pub fn build_rule_authoring_model(&self) -> editor_ui_model::RuleAuthoringModel {
        let Some(session) = &self.active_project_session else {
            return editor_ui_model::RuleAuthoringModel::empty();
        };
        let selected_path = self
            .selected_project_browser_path
            .as_ref()
            .filter(|path| is_rule_asset_relative_path(path))
            .cloned();
        RuleAuthoringService::build_model_with_selection(
            Some(&session.project_root),
            selected_path,
            self.selected_rule_card_id.clone(),
            self.selected_rule_graph_node_id.clone(),
        )
    }

    fn build_project_authoring_workspace_model(&self) -> ProjectAuthoringWorkspaceModel {
        let Some(session) = &self.active_project_session else {
            return ProjectAuthoringWorkspaceModel::empty();
        };

        let project_root = session.project_root.display().to_string();
        let active_scene_id = self
            .editor_scene_document
            .as_ref()
            .map(|scene| scene.scene_id.clone());
        let active_document =
            self.editor_scene_document
                .as_ref()
                .map(|scene| WorkspaceDocumentSummary {
                    document_kind: "scene".to_string(),
                    document_id: Some(scene.scene_id.clone()),
                    path: self
                        .selected_project_browser_path
                        .clone()
                        .or_else(|| Some(session.manifest.default_scene.clone())),
                    dirty: scene.dirty_state.dirty,
                });
        let mut domains = Vec::new();
        domains.push(self.project_workspace_domain(session));
        domains.push(self.scene_workspace_domain());
        domains.push(self.asset_workspace_domain());
        domains.push(self.directory_workspace_domain(
            session,
            WorkspaceDomainKind::Prefab,
            "Prefab",
            "Prefabs",
            &["prefab.json", "prefab"],
        ));
        domains.push(self.rule_workspace_domain(session));
        domains.push(self.directory_workspace_domain(
            session,
            WorkspaceDomainKind::Aui,
            "AUI",
            "AUI",
            &["aui.json", "ui.json"],
        ));
        domains.push(self.input_workspace_domain(session));
        domains.push(self.play_workspace_domain());
        domains.push(self.build_workspace_domain());
        domains.push(self.report_workspace_domain());

        let dirty_domains = domains
            .iter()
            .filter(|domain| domain.dirty)
            .map(|domain| domain.kind)
            .collect::<Vec<_>>();
        let selection = self.workspace_selection_summary();
        let diagnostics = workspace_diagnostics_from_editor_diagnostics(&self.diagnostics);
        let report = WorkspaceReportSummary {
            project_status: "open".to_string(),
            dirty_domains: dirty_domains.clone(),
            diagnostics: diagnostics.clone(),
            report_count: 0,
            evidence_count: 0,
            next_action_count: 0,
            last_command: None,
            last_transaction: None,
            build_status: self
                .last_desktop_export_report
                .as_ref()
                .map(|report| format!("{:?}", report.status).to_lowercase()),
            play_status: Some(
                if matches!(
                    self.play_session_controller.state(),
                    PlaySessionState::Running
                ) {
                    "running"
                } else {
                    "stopped"
                }
                .to_string(),
            ),
        };

        ProjectAuthoringWorkspaceModel {
            project_root: Some(project_root),
            project_id: Some(session.manifest.project_id.clone()),
            active_scene_id,
            active_document,
            selection,
            domains,
            dirty_domains,
            diagnostics,
            empty_message: String::new(),
            report,
        }
    }

    fn project_workspace_domain(&self, session: &ProjectSession) -> WorkspaceDomainSummary {
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Project,
            "Project",
            WorkspaceDomainStatus::Ready,
            format!(
                "project_id={} name={}",
                session.manifest.project_id, session.manifest.project_name
            ),
        );
        domain.item_count = 1;
        domain.active_document_path = Some("project.aife.json".to_string());
        domain
    }

    fn scene_workspace_domain(&self) -> WorkspaceDomainSummary {
        let Some(scene) = &self.editor_scene_document else {
            return self.empty_workspace_domain(
                WorkspaceDomainKind::Scene,
                "Scene",
                "no scene document open",
            );
        };
        let entity_count = scene.entities.len();
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Scene,
            "Scene",
            if scene.dirty_state.dirty {
                WorkspaceDomainStatus::Dirty
            } else {
                WorkspaceDomainStatus::Ready
            },
            format!("scene_id={} entity_count={entity_count}", scene.scene_id),
        );
        domain.item_count = entity_count;
        domain.dirty = scene.dirty_state.dirty;
        domain.selected_id = self.scene_selection.primary_entity_id.clone();
        domain.active_document_path = self.selected_project_browser_path.clone();
        domain
    }

    fn asset_workspace_domain(&self) -> WorkspaceDomainSummary {
        let browser = self.build_asset_browser_model(AssetQuery::default());
        let asset_count = browser.report.asset_count;
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Asset,
            "Asset",
            if browser.project_root.is_some() {
                WorkspaceDomainStatus::Ready
            } else {
                WorkspaceDomainStatus::NotConfigured
            },
            format!(
                "asset_browser_entries={} asset_count={asset_count}",
                browser.entries.len()
            ),
        );
        domain.item_count = asset_count;
        domain.selected_id = browser.selection.primary_path;
        domain
    }

    fn input_workspace_domain(&self, session: &ProjectSession) -> WorkspaceDomainSummary {
        let model = self.build_input_mapping_authoring_model();
        let status = match model.report.validation_status {
            editor_ui_model::InputMappingValidationStatus::Missing => WorkspaceDomainStatus::Empty,
            editor_ui_model::InputMappingValidationStatus::Ok => WorkspaceDomainStatus::Ready,
            editor_ui_model::InputMappingValidationStatus::Warning => {
                WorkspaceDomainStatus::Warning
            }
            editor_ui_model::InputMappingValidationStatus::Error => WorkspaceDomainStatus::Error,
        };
        let default = model.mapping_id.as_deref().unwrap_or("none");
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Input,
            "Input",
            if session.project_root.join("Input").exists() {
                status
            } else {
                WorkspaceDomainStatus::Empty
            },
            format!(
                "Input item_count={} default={} validation={:?}",
                model.report.mapping_count, default, model.report.validation_status
            ),
        );
        domain.item_count = model.report.mapping_count;
        domain.selected_id = model.mapping_id;
        domain.active_document_path = model.selected_path;
        domain.diagnostics.error_count = model
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::InputMappingDiagnosticSeverity::Error
            })
            .count();
        domain.diagnostics.warning_count = model
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::InputMappingDiagnosticSeverity::Warning
            })
            .count();
        domain
    }

    fn rule_workspace_domain(&self, session: &ProjectSession) -> WorkspaceDomainSummary {
        let model = self.build_rule_authoring_model();
        let manifest_count =
            count_matching_files(&session.project_root.join("Rules"), &["rule-manifest.json"]);
        let total_count = model.rule_count + manifest_count;
        let status = if model.rule_count > 0 {
            if model.document.report.diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == editor_ui_model::RuleAuthoringDiagnosticSeverity::Error
            }) {
                WorkspaceDomainStatus::Error
            } else {
                WorkspaceDomainStatus::Ready
            }
        } else if manifest_count > 0 {
            WorkspaceDomainStatus::Warning
        } else {
            WorkspaceDomainStatus::Empty
        };
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Rule,
            "Rule",
            status,
            format!(
                "Rule authoring_assets={} runtime_manifests={} selected={}",
                model.rule_count,
                manifest_count,
                model.selected_path.as_deref().unwrap_or("none")
            ),
        );
        domain.item_count = total_count;
        domain.selected_id = model.document.rule_id.clone();
        domain.active_document_path = model
            .selected_path
            .clone()
            .or_else(|| Some("Rules".to_string()));
        domain.diagnostics.error_count = model
            .document
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::RuleAuthoringDiagnosticSeverity::Error
            })
            .count();
        domain.diagnostics.warning_count = model
            .document
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == editor_ui_model::RuleAuthoringDiagnosticSeverity::Warning
            })
            .count();
        domain
    }

    fn play_workspace_domain(&self) -> WorkspaceDomainSummary {
        let status = if self.runtime_package.is_some() || self.active_project_session.is_some() {
            WorkspaceDomainStatus::Ready
        } else {
            WorkspaceDomainStatus::Empty
        };
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Play,
            "Play",
            status,
            if let Some(game_view) = &self.last_game_view_present_report {
                format!(
                    "gameview status={:?} frames={} descriptor={} input={}",
                    game_view.status,
                    game_view.frame_count,
                    game_view.texture_descriptor_status,
                    game_view.input_bridge_status
                )
            } else if let Some(report) = &self.last_editor_preview_package_report {
                format!(
                    "preview_package cache={} dirty={:?} duration_ms={}",
                    report.cache_status.as_report_str(),
                    report.dirty_domain_labels(),
                    report.duration_total_ms
                )
            } else if self.runtime_package.is_some() {
                "runtime package loaded".to_string()
            } else if self.active_project_session.is_some() {
                "project open; preview package will be prepared on Play".to_string()
            } else {
                "no project or runtime package loaded".to_string()
            },
        );
        domain.item_count = usize::from(self.runtime_package.is_some())
            + usize::from(self.last_editor_preview_package_report.is_some())
            + usize::from(self.last_game_view_present_report.is_some());
        domain
    }

    fn build_workspace_domain(&self) -> WorkspaceDomainSummary {
        let build = self.build_export_model();
        let status = match build
            .last_report
            .as_ref()
            .map(|report| report.status.as_str())
        {
            Some("success") => WorkspaceDomainStatus::Ready,
            Some("failed") => WorkspaceDomainStatus::Error,
            Some(_) => WorkspaceDomainStatus::Warning,
            None if build.selected_profile_id.is_some() => WorkspaceDomainStatus::Empty,
            None => WorkspaceDomainStatus::NotConfigured,
        };
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Build,
            "Build",
            status,
            build
                .last_report
                .as_ref()
                .map(|report| {
                    format!(
                        "profile={} status={} diagnostics={}",
                        report.profile, report.status, report.diagnostic_count
                    )
                })
                .unwrap_or_else(|| "no export report yet".to_string()),
        );
        domain.item_count = build.profiles.len();
        domain.selected_id = build.selected_profile_id;
        if let Some(report) = build.last_report {
            domain.active_document_path = Some(report.report_path);
            domain.diagnostics.error_count = usize::from(status == WorkspaceDomainStatus::Error);
        }
        domain
    }

    fn report_workspace_domain(&self) -> WorkspaceDomainSummary {
        let diagnostics = workspace_diagnostics_from_editor_diagnostics(&self.diagnostics);
        let status = if diagnostics.error_count > 0 {
            WorkspaceDomainStatus::Error
        } else if diagnostics.warning_count > 0 {
            WorkspaceDomainStatus::Warning
        } else {
            WorkspaceDomainStatus::Ready
        };
        let mut domain = WorkspaceDomainSummary::new(
            WorkspaceDomainKind::Report,
            "Report",
            status,
            format!(
                "diagnostics info={} warning={} error={}",
                diagnostics.info_count, diagnostics.warning_count, diagnostics.error_count
            ),
        );
        domain.item_count =
            diagnostics.info_count + diagnostics.warning_count + diagnostics.error_count;
        domain.diagnostics = diagnostics;
        domain
    }

    fn empty_workspace_domain(
        &self,
        kind: WorkspaceDomainKind,
        label: impl Into<String>,
        summary: impl Into<String>,
    ) -> WorkspaceDomainSummary {
        WorkspaceDomainSummary::new(kind, label, WorkspaceDomainStatus::Empty, summary)
    }

    fn directory_workspace_domain(
        &self,
        session: &ProjectSession,
        kind: WorkspaceDomainKind,
        label: impl Into<String>,
        relative_dir: &str,
        extensions_or_names: &[&str],
    ) -> WorkspaceDomainSummary {
        let dir = session.project_root.join(relative_dir);
        if !dir.exists() {
            return WorkspaceDomainSummary::new(
                kind,
                label,
                WorkspaceDomainStatus::Empty,
                format!("{relative_dir} directory not configured"),
            );
        }
        let item_count = count_matching_files(&dir, extensions_or_names);
        let mut domain = WorkspaceDomainSummary::new(
            kind,
            label,
            if item_count > 0 {
                WorkspaceDomainStatus::Ready
            } else {
                WorkspaceDomainStatus::Empty
            },
            format!("{relative_dir} item_count={item_count}"),
        );
        domain.item_count = item_count;
        domain.active_document_path = Some(relative_dir.to_string());
        domain
    }

    fn workspace_selection_summary(&self) -> WorkspaceSelectionSummary {
        if let Some(selection) = &self.selected_aui_node {
            return WorkspaceSelectionSummary {
                primary: Some(selection.clone()),
                secondary: Vec::new(),
            };
        }
        if let Some(entity_id) = &self.scene_selection.primary_entity_id {
            return WorkspaceSelectionSummary {
                primary: Some(WorkspaceSelectionTarget::Entity {
                    entity_id: entity_id.clone(),
                }),
                secondary: Vec::new(),
            };
        }
        if let Some(asset_ref) = &self.selected_project_browser_path {
            if is_rule_asset_relative_path(asset_ref) {
                let rule_id = self
                    .active_project_session
                    .as_ref()
                    .and_then(|session| {
                        RuleAuthoringService::load(&session.project_root, asset_ref).ok()
                    })
                    .map(|asset| asset.rule_id)
                    .unwrap_or_else(|| asset_ref.clone());
                return WorkspaceSelectionSummary {
                    primary: Some(WorkspaceSelectionTarget::Rule { rule_id }),
                    secondary: Vec::new(),
                };
            }
            return WorkspaceSelectionSummary {
                primary: Some(WorkspaceSelectionTarget::Asset {
                    asset_ref: asset_ref.clone(),
                }),
                secondary: Vec::new(),
            };
        }
        WorkspaceSelectionSummary::default()
    }

    fn build_export_model(&self) -> BuildExportModel {
        let Some(session) = &self.active_project_session else {
            let mut model = BuildExportModel::empty();
            model.commands = vec![
                BuildExportCommand::new(
                    "export_desktop_package",
                    "Export",
                    false,
                    Some("Open a project first.".to_string()),
                ),
                BuildExportCommand::new(
                    "build_and_run_desktop_package",
                    "Build & Run",
                    false,
                    Some("Open a project first.".to_string()),
                ),
                BuildExportCommand::new(
                    "build_release_package",
                    "Build Release",
                    false,
                    Some("Open a project first.".to_string()),
                ),
                BuildExportCommand::new(
                    "begin_asset_pick",
                    "Pick Icon",
                    false,
                    Some("Open a project first.".to_string()),
                ),
                BuildExportCommand::new(
                    "save_release_profile",
                    "Save Profile",
                    false,
                    Some("Open a project first.".to_string()),
                ),
                BuildExportCommand::new(
                    "open_build_output",
                    "Output",
                    false,
                    Some("Run Export first.".to_string()),
                ),
                BuildExportCommand::new(
                    "open_build_report",
                    "Report",
                    false,
                    Some("Run Export first.".to_string()),
                ),
            ];
            return model;
        };
        let project_root = &session.project_root;
        let output_dir = project_root.join("Build").join("Windows").join("dev");
        let has_report = self.last_desktop_export_report.is_some();
        let release_profile = self.release_profile_cache.as_ref().and_then(|profile| {
            let application = profile.application.as_ref()?;
            let architecture = profile.architecture.as_deref().unwrap_or("x86_64");
            Some(ReleaseBuildProfileModel {
                profile_id: "windows-release".to_string(),
                display_name: application.display_name.clone(),
                executable_name: application.executable_name.clone(),
                company_name: application.company_name.clone(),
                file_description: application.file_description.clone(),
                display_version: application.display_version.clone(),
                architecture: architecture.to_string(),
                icon_asset_id: application.icon.asset_id.clone(),
                output_preview: format!(
                    "Build/Windows/{architecture}/release/{}",
                    application.executable_name
                ),
                dirty: self.release_profile_dirty,
                validation_diagnostics: profile
                    .validation_issues()
                    .into_iter()
                    .map(|issue| format!("{}: {}", issue.field, issue.message))
                    .collect(),
            })
        });
        let release_valid = release_profile
            .as_ref()
            .is_some_and(|profile| profile.validation_diagnostics.is_empty());
        let mut profiles = vec![BuildProfileSummary {
            profile_id: "windows-dev".to_string(),
            label: "Windows Dev".to_string(),
            target: "windows".to_string(),
            output_dir: output_dir.display().to_string(),
            active: true,
        }];
        if let Some(release) = &release_profile {
            profiles.push(BuildProfileSummary {
                profile_id: release.profile_id.clone(),
                label: "Windows Release".to_string(),
                target: format!("windows/{}", release.architecture),
                output_dir: release.output_preview.clone(),
                active: false,
            });
        }
        BuildExportModel {
            selected_profile_id: Some("windows-dev".to_string()),
            profiles,
            release_profile,
            commands: vec![
                BuildExportCommand::new("export_desktop_package", "Export", true, None),
                BuildExportCommand::new("build_and_run_desktop_package", "Build & Run", true, None),
                BuildExportCommand::new(
                    "build_release_package",
                    "Build Release",
                    release_valid && !self.release_profile_dirty,
                    (!release_valid)
                        .then(|| "Load and validate windows.release profile first.".to_string())
                        .or_else(|| {
                            self.release_profile_dirty
                                .then(|| "Save Release Profile first.".to_string())
                        }),
                ),
                BuildExportCommand::new(
                    "begin_asset_pick",
                    "Pick Icon",
                    release_valid,
                    (!release_valid)
                        .then(|| "Load and validate windows.release profile first.".to_string()),
                ),
                BuildExportCommand::new(
                    "save_release_profile",
                    "Save Profile",
                    self.release_profile_dirty,
                    (!self.release_profile_dirty)
                        .then(|| "Release profile has no draft changes.".to_string()),
                ),
                BuildExportCommand::new(
                    "open_build_output",
                    "Output",
                    has_report,
                    (!has_report).then(|| "Run Export first.".to_string()),
                ),
                BuildExportCommand::new(
                    "open_build_report",
                    "Report",
                    has_report,
                    (!has_report).then(|| "Run Export first.".to_string()),
                ),
            ],
            last_report: self
                .last_desktop_export_report
                .as_ref()
                .map(desktop_export_report_summary),
            last_release_report: self.last_release_package_report.as_ref().map(|report| {
                ReleasePackageReportSummary {
                    status: match report.status {
                        crate::ReleasePackageStatus::Success => "success".to_string(),
                        crate::ReleasePackageStatus::Failed => "failed".to_string(),
                    },
                    product_name: report.display_name.clone(),
                    display_version: report.display_version.clone(),
                    entrypoint: report.entrypoint.clone(),
                    release_payload_hash: report.release_payload_hash.clone(),
                    diagnostic_count: report.diagnostics.len(),
                    next_action: report.next_action.clone(),
                }
            }),
            empty_message: String::new(),
        }
    }

    fn build_hierarchy_model(&self) -> HierarchyModel {
        let visual_order = self.build_aui_scene_visual_order_model();
        let authoring_view = if visual_order.is_some() {
            HierarchyAuthoringView::VisualOrder
        } else {
            HierarchyAuthoringView::EntityTree
        };
        if let Some(instance) = &self.editor_runtime_play_instance {
            let world = instance.runtime_world();
            let mut children: HashMap<Option<String>, Vec<HierarchyNode>> = HashMap::new();
            for entity_id in world.entity_ids() {
                let Some(meta) = world.entity(entity_id) else {
                    continue;
                };
                children
                    .entry(
                        meta.hierarchy
                            .parent_id
                            .as_ref()
                            .map(|id| id.as_str().to_string()),
                    )
                    .or_default()
                    .push(HierarchyNode {
                        entity_id: meta.id.as_str().to_string(),
                        label: meta.name.clone(),
                        alive: meta.alive && meta.enabled,
                        children: Vec::new(),
                    });
            }
            fn attach(
                mut node: HierarchyNode,
                children: &mut HashMap<Option<String>, Vec<HierarchyNode>>,
            ) -> HierarchyNode {
                let child_nodes = children
                    .remove(&Some(node.entity_id.clone()))
                    .unwrap_or_default();
                node.children = child_nodes
                    .into_iter()
                    .map(|child| attach(child, children))
                    .collect();
                node
            }
            let roots = children
                .remove(&None)
                .unwrap_or_default()
                .into_iter()
                .map(|node| attach(node, &mut children))
                .collect();
            return HierarchyModel {
                scene_id: Some(instance.scene_id().to_string()),
                roots,
                selected_entity_id: self.selected_entity_id.clone().filter(|_| {
                    self.selected_entity_source
                        == Some(EntitySelectionSource::ActiveGameViewRuntime)
                }),
                authoring_view,
                visual_order,
                source_domain: HierarchySourceDomain::ActiveGameViewRuntime,
                status: format!(
                    "play_mode_active_runtime entity_count={} temporary_edit_count={}",
                    world.entity_count(),
                    instance.temporary_edit_summary().edited_field_count
                ),
            };
        }
        if let Some(document) = &self.editor_scene_document {
            let mut children: HashMap<Option<String>, Vec<HierarchyNode>> = HashMap::new();
            let mut entities = document.entities.iter().collect::<Vec<_>>();
            entities.sort_by(|left, right| {
                left.sibling_order
                    .cmp(&right.sibling_order)
                    .then_with(|| left.entity_id.cmp(&right.entity_id))
            });
            for entity in entities {
                children
                    .entry(entity.parent_id.clone())
                    .or_default()
                    .push(HierarchyNode {
                        entity_id: entity.entity_id.clone(),
                        label: entity.name.clone(),
                        alive: entity.enabled,
                        children: Vec::new(),
                    });
            }
            fn attach(
                mut node: HierarchyNode,
                children: &mut HashMap<Option<String>, Vec<HierarchyNode>>,
            ) -> HierarchyNode {
                let child_nodes = children
                    .remove(&Some(node.entity_id.clone()))
                    .unwrap_or_default();
                node.children = child_nodes
                    .into_iter()
                    .map(|child| attach(child, children))
                    .collect();
                node
            }
            let roots = children
                .remove(&None)
                .unwrap_or_default()
                .into_iter()
                .map(|node| attach(node, &mut children))
                .collect();
            return HierarchyModel {
                scene_id: Some(document.scene_id.clone()),
                roots,
                selected_entity_id: self.scene_selection.primary_entity_id.clone(),
                authoring_view,
                visual_order,
                source_domain: HierarchySourceDomain::AuthoringScene,
                status: "authoring_scene".to_string(),
            };
        }
        let Some(world) = &self.world else {
            return HierarchyModel {
                scene_id: None,
                roots: Vec::new(),
                selected_entity_id: self.selected_entity_id.clone(),
                authoring_view,
                visual_order,
                source_domain: HierarchySourceDomain::Empty,
                status: "empty".to_string(),
            };
        };
        let scene_id = self
            .runtime_package
            .as_ref()
            .map(|package| package.manifest.active_scene_id.clone());
        let mut children: HashMap<Option<String>, Vec<HierarchyNode>> = HashMap::new();
        for entity_id in world.entity_ids() {
            let Some(meta) = world.entity(entity_id) else {
                continue;
            };
            children
                .entry(
                    meta.hierarchy
                        .parent_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                )
                .or_default()
                .push(HierarchyNode {
                    entity_id: meta.id.as_str().to_string(),
                    label: meta.name.clone(),
                    alive: meta.alive && meta.enabled,
                    children: Vec::new(),
                });
        }
        fn attach(
            mut node: HierarchyNode,
            children: &mut HashMap<Option<String>, Vec<HierarchyNode>>,
        ) -> HierarchyNode {
            let child_nodes = children
                .remove(&Some(node.entity_id.clone()))
                .unwrap_or_default();
            node.children = child_nodes
                .into_iter()
                .map(|child| attach(child, children))
                .collect();
            node
        }
        let roots = children
            .remove(&None)
            .unwrap_or_default()
            .into_iter()
            .map(|node| attach(node, &mut children))
            .collect();
        HierarchyModel {
            scene_id,
            roots,
            selected_entity_id: self.selected_entity_id.clone(),
            authoring_view,
            visual_order,
            source_domain: HierarchySourceDomain::OpenedRuntimePackage,
            status: format!(
                "opened_runtime_package entity_count={}",
                world.entity_count()
            ),
        }
    }

    fn build_aui_scene_visual_order_model(
        &self,
    ) -> Option<editor_ui_model::SceneVisualOrderAuthoringModel> {
        let session = self.active_project_session.as_ref()?;
        let document_path = self.selected_aui_document_path()?;
        let full_path = session
            .project_root
            .join(normalize_project_relative_path(&document_path));
        let service = AuiAuthoringService::open(&full_path).ok()?;
        let scene_path = self
            .scene_path
            .as_ref()
            .map(|path| path.display().to_string());
        Some(
            AuiSceneAuthoringService::build_document_overlay(
                scene_path,
                document_path,
                service.document(),
                AuiSceneViewProjection::Orthographic2D,
                self.selected_aui_node.as_ref(),
            )
            .visual_order,
        )
    }

    fn selected_aui_document_path(&self) -> Option<String> {
        if let Some(WorkspaceSelectionTarget::AuiNode { document_path, .. }) =
            &self.selected_aui_node
        {
            return Some(document_path.clone());
        }
        self.selected_project_browser_path
            .as_ref()
            .filter(|path| is_aui_document_relative_path(path))
            .cloned()
    }

    fn build_inspector_model(&self) -> InspectorModel {
        if self.selected_aui_node.is_some() {
            return self.build_aui_node_inspector_model();
        }
        if self.selected_entity_source == Some(EntitySelectionSource::ActiveGameViewRuntime) {
            let Some(selected) = &self.selected_entity_id else {
                return InspectorModel {
                    selected_entity_id: None,
                    title: "No Runtime Selection".to_string(),
                    sections: Vec::new(),
                    readonly: true,
                    persistence: InspectorPersistence::ReadOnly,
                };
            };
            let Some(instance) = &self.editor_runtime_play_instance else {
                return InspectorModel {
                    selected_entity_id: Some(selected.clone()),
                    title: "Missing Runtime Instance".to_string(),
                    sections: Vec::new(),
                    readonly: true,
                    persistence: InspectorPersistence::ReadOnly,
                };
            };
            return self.build_runtime_entity_inspector_model(
                instance.runtime_world(),
                selected,
                EntitySelectionSource::ActiveGameViewRuntime,
            );
        }
        if self.editor_scene_document.is_some() {
            let Some(selected) = &self.scene_selection.primary_entity_id else {
                return InspectorModel {
                    selected_entity_id: None,
                    title: "No Selection".to_string(),
                    sections: Vec::new(),
                    readonly: true,
                    persistence: InspectorPersistence::ReadOnly,
                };
            };
            return self
                .build_authoring_entity_inspector_model(selected)
                .unwrap_or_else(|| InspectorModel {
                    selected_entity_id: None,
                    title: "Missing Entity".to_string(),
                    sections: Vec::new(),
                    readonly: true,
                    persistence: InspectorPersistence::ReadOnly,
                });
        }
        let Some(selected) = &self.selected_entity_id else {
            return InspectorModel {
                selected_entity_id: None,
                title: "No Selection".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        let Some(world) = &self.world else {
            return InspectorModel {
                selected_entity_id: None,
                title: "No Runtime Package".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        self.build_runtime_entity_inspector_model(
            world,
            selected,
            self.selected_entity_source
                .unwrap_or(EntitySelectionSource::OpenedRuntimePackage),
        )
    }

    fn build_authoring_entity_inspector_model(&self, selected: &str) -> Option<InspectorModel> {
        let document = self.editor_scene_document.as_ref()?;
        let entity = document.entity(selected)?;
        let mut sections = Vec::new();
        if let Some(transform) = entity.transform {
            sections.push(InspectorSection {
                section_id: "transform".to_string(),
                title: "Transform".to_string(),
                fields: vec![
                    editable_field(
                        "transform.localPosition",
                        "localPosition",
                        InspectorValue::Vec3(editor_vec3_to_ui(transform.local_position)),
                        InspectorValueType::Vec3,
                    ),
                    editable_field(
                        "transform.localRotation",
                        "localRotation",
                        InspectorValue::Vec3(editor_vec3_to_ui(transform.local_rotation)),
                        InspectorValueType::Vec3,
                    ),
                    editable_field(
                        "transform.localScale",
                        "localScale",
                        InspectorValue::Vec3(editor_vec3_to_ui(transform.local_scale)),
                        InspectorValueType::Vec3,
                    ),
                ],
            });
        }
        for component in &entity.components {
            let fields = if component.component_type == "SpriteRenderer2D" {
                vec![asset_ref_picker_field(
                    "components.SpriteRenderer2D.spriteRef",
                    "spriteRef",
                    component
                        .fields
                        .get("spriteRef")
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok())
                        .map(InspectorValue::AssetRef)
                        .unwrap_or(InspectorValue::Empty),
                )]
            } else {
                vec![editable_field(
                    &format!("components.{}", component.component_type),
                    "fields",
                    InspectorValue::Json(component.fields.clone()),
                    InspectorValueType::Json,
                )]
            };
            sections.push(InspectorSection {
                section_id: component.component_type.clone(),
                title: component.component_type.clone(),
                fields,
            });
        }
        if let Some(mesh) = &entity.mesh {
            sections.push(InspectorSection {
                section_id: "mesh".to_string(),
                title: "Mesh".to_string(),
                fields: vec![
                    readonly_field(
                        "mesh.assetRef",
                        "assetRef",
                        mesh.asset_ref
                            .as_ref()
                            .cloned()
                            .map(InspectorValue::AssetRef)
                            .unwrap_or(InspectorValue::Empty),
                        if mesh.asset_ref.is_some() {
                            InspectorValueType::AssetRef
                        } else {
                            InspectorValueType::Empty
                        },
                    ),
                    readonly_field(
                        "mesh.visible",
                        "visible",
                        InspectorValue::Bool(mesh.visible),
                        InspectorValueType::Bool,
                    ),
                ],
            });
        }
        sections.push(InspectorSection {
            section_id: "metadata".to_string(),
            title: "Metadata".to_string(),
            fields: vec![
                readonly_field(
                    "metadata.entityId",
                    "entityId",
                    InspectorValue::EntityRef(entity.entity_id.clone()),
                    InspectorValueType::EntityRef,
                ),
                readonly_field(
                    "metadata.kind",
                    "kind",
                    InspectorValue::String(entity.kind.clone()),
                    InspectorValueType::String,
                ),
                readonly_field(
                    "metadata.enabled",
                    "enabled",
                    InspectorValue::Bool(entity.enabled),
                    InspectorValueType::Bool,
                ),
            ],
        });
        Some(InspectorModel {
            selected_entity_id: Some(selected.to_string()),
            title: entity.name.clone(),
            sections,
            readonly: false,
            persistence: InspectorPersistence::PersistentAuthoring,
        })
    }

    fn build_runtime_entity_inspector_model(
        &self,
        world: &World,
        selected: &str,
        source: EntitySelectionSource,
    ) -> InspectorModel {
        let entity_id = EntityId::new(selected.to_string());
        let Some(meta) = world.entity(&entity_id) else {
            return InspectorModel {
                selected_entity_id: None,
                title: "Missing Runtime Entity".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        let runtime_temporary_editable = source == EntitySelectionSource::ActiveGameViewRuntime;
        let mut sections = Vec::new();
        if let Some(transform) = world.transform(&entity_id) {
            sections.push(InspectorSection {
                section_id: "transform".to_string(),
                title: "Transform".to_string(),
                fields: vec![
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "transform.localPosition",
                        "localPosition",
                        InspectorValue::Vec3(runtime_vec3_to_ui(transform.local_position)),
                        InspectorValueType::Vec3,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "transform.localRotation",
                        "localRotation",
                        InspectorValue::Vec3(runtime_vec3_to_ui(transform.local_rotation)),
                        InspectorValueType::Vec3,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "transform.localScale",
                        "localScale",
                        InspectorValue::Vec3(runtime_vec3_to_ui(transform.local_scale)),
                        InspectorValueType::Vec3,
                    ),
                ],
            });
        }
        if let Some(renderable) = world.renderable(&entity_id) {
            sections.push(InspectorSection {
                section_id: "renderable".to_string(),
                title: "Renderable".to_string(),
                fields: vec![
                    readonly_field(
                        "renderable.meshRef",
                        "meshRef",
                        renderable
                            .mesh_ref
                            .clone()
                            .map(|asset_id| {
                                InspectorValue::AssetRef(editor_ui_model::EditorAssetRef::new(
                                    asset_id, "mesh",
                                ))
                            })
                            .unwrap_or(InspectorValue::Empty),
                        if renderable.mesh_ref.is_some() {
                            InspectorValueType::AssetRef
                        } else {
                            InspectorValueType::Empty
                        },
                    ),
                    readonly_field(
                        "renderable.materialRef",
                        "materialRef",
                        renderable
                            .material_ref
                            .clone()
                            .map(|asset_id| {
                                InspectorValue::AssetRef(editor_ui_model::EditorAssetRef::new(
                                    asset_id, "material",
                                ))
                            })
                            .unwrap_or(InspectorValue::Empty),
                        if renderable.material_ref.is_some() {
                            InspectorValueType::AssetRef
                        } else {
                            InspectorValueType::Empty
                        },
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "renderable.visible",
                        "visible",
                        InspectorValue::Bool(renderable.visible),
                        InspectorValueType::Bool,
                    ),
                    readonly_field(
                        "renderable.layer",
                        "layer",
                        InspectorValue::String(renderable.layer.clone()),
                        InspectorValueType::String,
                    ),
                ],
            });
        }
        if let Some(sprite) = world.sprite_renderer2d(&entity_id) {
            sections.push(InspectorSection {
                section_id: "sprite_renderer2d".to_string(),
                title: "SpriteRenderer2D".to_string(),
                fields: vec![
                    readonly_field(
                        "spriteRenderer2D.spriteRef",
                        "spriteRef",
                        sprite
                            .sprite_ref
                            .clone()
                            .map(|asset_id| {
                                InspectorValue::AssetRef(editor_ui_model::EditorAssetRef::new(
                                    asset_id, "sprite",
                                ))
                            })
                            .unwrap_or(InspectorValue::Empty),
                        if sprite.sprite_ref.is_some() {
                            InspectorValueType::AssetRef
                        } else {
                            InspectorValueType::Empty
                        },
                    ),
                    readonly_field(
                        "spriteRenderer2D.materialRef",
                        "materialRef",
                        sprite
                            .material_ref
                            .clone()
                            .map(|asset_id| {
                                InspectorValue::AssetRef(editor_ui_model::EditorAssetRef::new(
                                    asset_id, "material",
                                ))
                            })
                            .unwrap_or(InspectorValue::Empty),
                        if sprite.material_ref.is_some() {
                            InspectorValueType::AssetRef
                        } else {
                            InspectorValueType::Empty
                        },
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "spriteRenderer2D.color",
                        "color",
                        InspectorValue::Json(serde_json::json!({
                            "r": sprite.color[0],
                            "g": sprite.color[1],
                            "b": sprite.color[2],
                            "a": sprite.color[3]
                        })),
                        InspectorValueType::Json,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "spriteRenderer2D.visible",
                        "visible",
                        InspectorValue::Bool(sprite.visible),
                        InspectorValueType::Bool,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "spriteRenderer2D.sortingLayer",
                        "sortingLayer",
                        InspectorValue::Number(sprite.sorting_layer as f64),
                        InspectorValueType::Number,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "spriteRenderer2D.orderInLayer",
                        "orderInLayer",
                        InspectorValue::Number(sprite.order_in_layer as f64),
                        InspectorValueType::Number,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "spriteRenderer2D.sortZ",
                        "sortZ",
                        InspectorValue::Number(sprite.sort_z as f64),
                        InspectorValueType::Number,
                    ),
                ],
            });
        }
        if let Some(collider) = world.collider2d(&entity_id) {
            let shape = match collider.shape {
                Shape2D::Aabb { half_extents } => serde_json::json!({
                    "kind": "aabb",
                    "halfExtents": { "x": half_extents.x, "y": half_extents.y }
                }),
                Shape2D::Circle { radius } => serde_json::json!({
                    "kind": "circle",
                    "radius": radius
                }),
            };
            sections.push(InspectorSection {
                section_id: "collider2d".to_string(),
                title: "Collider2D".to_string(),
                fields: vec![
                    readonly_field(
                        "collider2D.shape",
                        "shape",
                        InspectorValue::Json(shape),
                        InspectorValueType::Json,
                    ),
                    readonly_field(
                        "collider2D.offset",
                        "offset",
                        InspectorValue::Json(serde_json::json!({
                            "x": collider.offset.x,
                            "y": collider.offset.y
                        })),
                        InspectorValueType::Json,
                    ),
                    runtime_editable_field(
                        runtime_temporary_editable,
                        "collider2D.enabled",
                        "enabled",
                        InspectorValue::Bool(collider.enabled),
                        InspectorValueType::Bool,
                    ),
                    readonly_field(
                        "collider2D.isSensor",
                        "isSensor",
                        InspectorValue::Bool(collider.is_sensor),
                        InspectorValueType::Bool,
                    ),
                ],
            });
        }
        sections.push(InspectorSection {
            section_id: "metadata".to_string(),
            title: "Metadata".to_string(),
            fields: vec![
                readonly_field(
                    "metadata.entityId",
                    "entityId",
                    InspectorValue::EntityRef(meta.id.as_str().to_string()),
                    InspectorValueType::EntityRef,
                ),
                readonly_field(
                    "metadata.kind",
                    "kind",
                    InspectorValue::String(meta.kind.clone()),
                    InspectorValueType::String,
                ),
                readonly_field(
                    "metadata.enabled",
                    "enabled",
                    InspectorValue::Bool(meta.enabled),
                    InspectorValueType::Bool,
                ),
                readonly_field(
                    "metadata.selectionSource",
                    "selectionSource",
                    InspectorValue::String(source.as_str().to_string()),
                    InspectorValueType::String,
                ),
                readonly_field(
                    "metadata.readonly",
                    "readonly",
                    InspectorValue::Bool(!runtime_temporary_editable),
                    InspectorValueType::Bool,
                ),
                readonly_field(
                    "metadata.persistence",
                    "persistence",
                    InspectorValue::String(if runtime_temporary_editable {
                        "temporary_play_session".to_string()
                    } else {
                        "read_only_runtime_package".to_string()
                    }),
                    InspectorValueType::String,
                ),
            ],
        });
        InspectorModel {
            selected_entity_id: Some(selected.to_string()),
            title: if source == EntitySelectionSource::ActiveGameViewRuntime {
                format!("Runtime / Temporary: {}", meta.name)
            } else {
                meta.name.clone()
            },
            sections,
            readonly: !runtime_temporary_editable,
            persistence: if runtime_temporary_editable {
                InspectorPersistence::TemporaryPlaySession
            } else {
                InspectorPersistence::ReadOnlyRuntimePackage
            },
        }
    }

    fn build_aui_node_inspector_model(&self) -> InspectorModel {
        let Some(WorkspaceSelectionTarget::AuiNode {
            document_path,
            document_id,
            node_id,
        }) = &self.selected_aui_node
        else {
            return InspectorModel {
                selected_entity_id: None,
                title: "No AUI Selection".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        let Some(session) = &self.active_project_session else {
            return InspectorModel {
                selected_entity_id: None,
                title: "Missing Project".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        let full_path = session
            .project_root
            .join(normalize_project_relative_path(document_path));
        let Ok(service) = AuiAuthoringService::open(&full_path) else {
            return InspectorModel {
                selected_entity_id: None,
                title: "Missing AUI Document".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };
        let Some(node) = service
            .document()
            .nodes
            .iter()
            .find(|node| node.node_id == *node_id)
        else {
            return InspectorModel {
                selected_entity_id: None,
                title: "Missing AUI Node".to_string(),
                sections: Vec::new(),
                readonly: true,
                persistence: InspectorPersistence::ReadOnly,
            };
        };

        InspectorModel {
            selected_entity_id: None,
            title: format!("AUI Node: {}", node.name),
            readonly: false,
            persistence: InspectorPersistence::PersistentAuthoring,
            sections: vec![
                InspectorSection {
                    section_id: "aui.identity".to_string(),
                    title: "AUI Node".to_string(),
                    fields: vec![
                        readonly_field(
                            "aui.documentPath",
                            "document_path",
                            InspectorValue::String(document_path.clone()),
                            InspectorValueType::String,
                        ),
                        readonly_field(
                            "aui.documentId",
                            "document_id",
                            InspectorValue::String(document_id.clone()),
                            InspectorValueType::String,
                        ),
                        readonly_field(
                            "aui.nodeId",
                            "node_id",
                            InspectorValue::String(node.node_id.clone()),
                            InspectorValueType::String,
                        ),
                        editable_field(
                            "aui.name",
                            "name",
                            InspectorValue::String(node.name.clone()),
                            InspectorValueType::String,
                        ),
                        readonly_field(
                            "aui.kind",
                            "kind",
                            InspectorValue::String(format!("{:?}", node.kind)),
                            InspectorValueType::String,
                        ),
                    ],
                },
                InspectorSection {
                    section_id: "aui.behavior".to_string(),
                    title: "Behavior".to_string(),
                    fields: vec![
                        editable_field(
                            "aui.text",
                            "text",
                            node.text
                                .clone()
                                .map(InspectorValue::String)
                                .unwrap_or(InspectorValue::Empty),
                            if node.text.is_some() {
                                InspectorValueType::String
                            } else {
                                InspectorValueType::Empty
                            },
                        ),
                        editable_field(
                            "aui.visible",
                            "visible",
                            InspectorValue::Bool(node.visible),
                            InspectorValueType::Bool,
                        ),
                        editable_field(
                            "aui.interactable",
                            "interactable",
                            InspectorValue::Bool(node.interactable),
                            InspectorValueType::Bool,
                        ),
                        editable_field(
                            "aui.consumeInput",
                            "consumeInput",
                            InspectorValue::Bool(node.consume_input),
                            InspectorValueType::Bool,
                        ),
                        asset_ref_picker_field(
                            "aui.image",
                            "image",
                            node.image
                                .as_ref()
                                .map(|image| {
                                    InspectorValue::AssetRef(editor_ui_model::EditorAssetRef::new(
                                        image.asset_id.clone(),
                                        "texture",
                                    ))
                                })
                                .unwrap_or(InspectorValue::Empty),
                        ),
                    ],
                },
                InspectorSection {
                    section_id: "aui.layout".to_string(),
                    title: "Layout".to_string(),
                    fields: vec![editable_field(
                        "aui.rect",
                        "rect",
                        InspectorValue::Json(
                            serde_json::to_value(node.rect).unwrap_or(serde_json::Value::Null),
                        ),
                        InspectorValueType::Json,
                    )],
                },
                InspectorSection {
                    section_id: "aui.style_binding_action".to_string(),
                    title: "Style / Binding / Action".to_string(),
                    fields: vec![
                        editable_field(
                            "aui.style",
                            "style",
                            InspectorValue::Json(
                                serde_json::to_value(&node.style)
                                    .unwrap_or(serde_json::Value::Null),
                            ),
                            InspectorValueType::Json,
                        ),
                        readonly_field(
                            "aui.bindingRefs",
                            "binding path",
                            InspectorValue::Json(
                                serde_json::to_value(&node.binding_refs)
                                    .unwrap_or(serde_json::Value::Null),
                            ),
                            InspectorValueType::Json,
                        ),
                        readonly_field(
                            "aui.actionRefs",
                            "action ref",
                            InspectorValue::Json(
                                serde_json::to_value(&node.action_refs)
                                    .unwrap_or(serde_json::Value::Null),
                            ),
                            InspectorValueType::Json,
                        ),
                    ],
                },
            ],
        }
    }

    fn build_viewport_model(&self) -> ViewportModel {
        if let Some(frame) = &self.last_game_view_runtime_frame {
            let renderables = self
                .world
                .as_ref()
                .map(|world| {
                    world
                        .alive_renderables()
                        .into_iter()
                        .map(|(entity_id, transform, renderable)| RenderableSummary {
                            entity_id: entity_id.as_str().to_string(),
                            mesh_ref: renderable.mesh_ref.clone(),
                            material_ref: renderable.material_ref.clone(),
                            local_position: runtime_vec3_to_ui(transform.local_position),
                            visible: renderable.visible,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            return ViewportModel {
                scene_id: Some(frame.scene_id.clone()),
                frame: frame.frame_index,
                frame_hash: Some(frame.frame_hash.clone()),
                texture_id: Some(frame.texture_id.clone()),
                target_id: Some(frame.target_id.clone()),
                renderable_count: frame.renderable_count,
                selected_entity: self.selected_entity_summary(),
                renderables,
                collider_overlay: self.build_collider_overlay_model(),
            };
        }
        if let Some(output) = &self.last_frame_output {
            return viewport_from_snapshot(
                &output.snapshot,
                Some(output.frame_hash.clone()),
                self.selected_entity_summary(),
            );
        }
        let scene_id = self
            .editor_scene_document
            .as_ref()
            .map(|document| document.scene_id.clone())
            .or_else(|| {
                self.runtime_package
                    .as_ref()
                    .map(|package| package.manifest.active_scene_id.clone())
            });
        let renderables = self
            .world
            .as_ref()
            .map(|world| {
                world
                    .alive_renderables()
                    .into_iter()
                    .map(|(entity_id, transform, renderable)| RenderableSummary {
                        entity_id: entity_id.as_str().to_string(),
                        mesh_ref: renderable.mesh_ref.clone(),
                        material_ref: renderable.material_ref.clone(),
                        local_position: runtime_vec3_to_ui(transform.local_position),
                        visible: renderable.visible,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ViewportModel {
            scene_id,
            frame: 0,
            frame_hash: None,
            texture_id: None,
            target_id: None,
            renderable_count: renderables.len(),
            selected_entity: self.selected_entity_summary(),
            renderables,
            collider_overlay: self.build_collider_overlay_model(),
        }
    }

    fn build_collider_overlay_model(&self) -> ColliderOverlayModel {
        let Some(document) = self.editor_scene_document.as_ref() else {
            return ColliderOverlayModel::default();
        };
        collider_debug_draw_list_to_overlay_model(&ColliderDebugDrawList::build(
            document,
            &self.scene_selection,
        ))
    }

    fn build_console_model(&self) -> ConsoleModel {
        ConsoleModel {
            unread_error_count: self
                .console_entries
                .iter()
                .filter(|entry| entry.level == ConsoleLevel::Error)
                .count() as u32,
            unread_warning_count: self
                .console_entries
                .iter()
                .filter(|entry| entry.level == ConsoleLevel::Warning)
                .count() as u32,
            entries: self.console_entries.clone(),
        }
    }

    fn build_runtime_trace_model(&self) -> RuntimeTraceModel {
        let Some(trace) = self.last_trace() else {
            return RuntimeTraceModel {
                frame: 0,
                entries: Vec::new(),
                selected_entry_id: self.selected_trace_entry_id.clone(),
            };
        };
        let frame = trace.events.last().map_or(0, |event| event.frame);
        RuntimeTraceModel {
            frame,
            entries: trace
                .events
                .iter()
                .enumerate()
                .map(|(index, event)| RuntimeTraceEntryView {
                    entry_id: trace_entry_id(index),
                    frame: event.frame,
                    phase: event.phase.clone(),
                    system_id: event.system_id.clone(),
                    message: event.message.clone(),
                    entity_id: None,
                    level: TraceLevel::Info,
                })
                .collect(),
            selected_entry_id: self.selected_trace_entry_id.clone(),
        }
    }

    fn build_ai_panel_model(&self) -> AiPanelModel {
        AiPanelModel {
            prompt_placeholder: "Describe an editor change...".to_string(),
            prompt_draft: self.ai_prompt_draft.clone(),
            messages: self.ai_panel_messages.clone(),
            gateway_access: Default::default(),
            proposed_commands: self.ai_proposed_commands.clone(),
            allowed_command_ids: vec![
                "select_scene_entity".to_string(),
                "create_scene_entity".to_string(),
                "delete_scene_entity".to_string(),
                "rename_scene_entity".to_string(),
                "set_scene_transform".to_string(),
                "set_scene_component_field".to_string(),
                "save_scene_document".to_string(),
                "undo_scene_edit".to_string(),
                "redo_scene_edit".to_string(),
                "place_asset_into_scene".to_string(),
                "create_rule_asset".to_string(),
                "open_rule_asset".to_string(),
                "select_rule_card".to_string(),
                "set_rule_card_field".to_string(),
                "add_rule_card".to_string(),
                "remove_rule_card".to_string(),
                "select_rule_graph_node".to_string(),
                "refresh_rule_graph_preview".to_string(),
                "set_rule_trigger".to_string(),
                "add_rule_statement".to_string(),
                "add_rule_operation".to_string(),
                "validate_rule_asset".to_string(),
                "build_rule_artifact".to_string(),
                "create_aui_document".to_string(),
                "open_aui_document".to_string(),
                "add_aui_node".to_string(),
                "set_aui_node_field".to_string(),
                "set_aui_binding_path".to_string(),
                "set_aui_action_ref".to_string(),
                "validate_aui_document".to_string(),
                "save_aui_document".to_string(),
                "preview_aui_overlay".to_string(),
                "import_project_patch".to_string(),
                "preview_imported_project_patch".to_string(),
                "apply_imported_project_patch".to_string(),
            ],
            busy: self.llm_request_controller.is_busy(),
            stage: self.ai_panel_stage,
            status_summary: self.ai_panel_status_summary.clone(),
        }
    }

    pub(crate) fn last_trace(&self) -> Option<&RuntimeTrace> {
        self.last_frame_output.as_ref().map(|output| &output.trace)
    }

    fn selected_entity_summary(&self) -> Option<EntitySummary> {
        let selected = self.selected_entity_id.as_ref()?;
        if let Some(document) = &self.editor_scene_document {
            let entity = document.entity(selected)?;
            return Some(EntitySummary {
                entity_id: entity.entity_id.clone(),
                label: entity.name.clone(),
            });
        }
        let world = self.world.as_ref()?;
        let entity_id = EntityId::new(selected.clone());
        let meta = world.entity(&entity_id)?;
        Some(EntitySummary {
            entity_id: meta.id.as_str().to_string(),
            label: meta.name.clone(),
        })
    }
}
fn toolbar_command(
    id: &str,
    label: &str,
    enabled: bool,
    reason_disabled: Option<String>,
) -> ToolbarCommand {
    ToolbarCommand {
        command_id: id.to_string(),
        label: label.to_string(),
        enabled,
        reason_disabled: if enabled { None } else { reason_disabled },
    }
}

fn is_aui_document_relative_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with(".aui.json") || normalized.ends_with(".ui.json")
}

fn readonly_field(
    field_id: &str,
    label: &str,
    value: InspectorValue,
    value_type: InspectorValueType,
) -> InspectorField {
    inspector_field(field_id, label, value, value_type, true)
}

fn editable_field(
    field_id: &str,
    label: &str,
    value: InspectorValue,
    value_type: InspectorValueType,
) -> InspectorField {
    inspector_field(field_id, label, value, value_type, false)
}

fn asset_ref_picker_field(field_id: &str, label: &str, value: InspectorValue) -> InspectorField {
    inspector_field(field_id, label, value, InspectorValueType::AssetRef, false)
}

fn runtime_editable_field(
    editable: bool,
    field_id: &str,
    label: &str,
    value: InspectorValue,
    value_type: InspectorValueType,
) -> InspectorField {
    if editable {
        editable_field(field_id, label, value, value_type)
    } else {
        readonly_field(field_id, label, value, value_type)
    }
}

fn inspector_field(
    field_id: &str,
    label: &str,
    value: InspectorValue,
    value_type: InspectorValueType,
    readonly: bool,
) -> InspectorField {
    InspectorField {
        field_id: field_id.to_string(),
        label: label.to_string(),
        value,
        value_type,
        path: field_id.to_string(),
        readonly,
        editable: !readonly,
    }
}

fn viewport_from_snapshot(
    snapshot: &RenderSnapshot,
    frame_hash: Option<String>,
    selected_entity: Option<EntitySummary>,
) -> ViewportModel {
    let renderables = snapshot
        .renderables
        .iter()
        .map(|renderable| RenderableSummary {
            entity_id: renderable.entity_id.clone(),
            mesh_ref: renderable.mesh_ref.clone(),
            material_ref: renderable.material_ref.clone(),
            local_position: runtime_vec3_to_ui(renderable.local_position),
            visible: renderable.visible,
        })
        .collect::<Vec<_>>();
    ViewportModel {
        scene_id: Some(snapshot.scene_id.clone()),
        frame: snapshot.frame,
        frame_hash,
        texture_id: None,
        target_id: None,
        renderable_count: renderables.len(),
        selected_entity,
        renderables,
        collider_overlay: ColliderOverlayModel::default(),
    }
}

fn collider_debug_draw_list_to_overlay_model(list: &ColliderDebugDrawList) -> ColliderOverlayModel {
    ColliderOverlayModel {
        collider_count: list.collider_count,
        draw_item_count: list.draw_item_count,
        selected_entity_id: list.selected_entity_id.clone(),
        invalid_collider_count: list.invalid_collider_count,
        missing_transform_count: list.missing_transform_count,
        draw_items: list
            .draw_items
            .iter()
            .map(|item| ColliderOverlayItem {
                entity_id: item.entity_id.clone(),
                shape: match item.shape {
                    ColliderDebugShape::Aabb { half_extents } => ColliderOverlayShape::Aabb {
                        half_extents: editor_vec3_to_ui(half_extents),
                    },
                    ColliderDebugShape::Circle { radius } => {
                        ColliderOverlayShape::Circle { radius }
                    }
                },
                center: editor_vec3_to_ui(item.center),
                enabled: item.enabled,
                sensor: item.sensor,
                selected: item.selected,
                layer: item.layer,
                mask: item.mask,
            })
            .collect(),
        diagnostics: list
            .diagnostics
            .iter()
            .map(|diagnostic| ColliderOverlayDiagnostic {
                severity: diagnostic.severity.clone(),
                entity_id: diagnostic.entity_id.clone(),
                component_type: diagnostic.component_type.clone(),
                field_path: diagnostic.field_path.clone(),
                message: diagnostic.message.clone(),
                suggestion: diagnostic.suggestion.clone(),
            })
            .collect(),
    }
}

pub(crate) fn trace_entry_id(index: usize) -> String {
    format!("trace-{}", index + 1)
}

fn desktop_export_report_summary(report: &DesktopExportReport) -> BuildExportReportSummary {
    BuildExportReportSummary {
        status: match report.status {
            DesktopExportStatus::Success => "success".to_string(),
            DesktopExportStatus::Failed => "failed".to_string(),
        },
        profile: report.profile.clone(),
        target: report.target.clone(),
        package_dir: report.package_dir.clone(),
        report_path: report_path_for_desktop_export(report),
        runtime_package_dir: report.runtime_package_dir.clone(),
        player_exit_code: report.player_exit_code,
        player_exit_reason: report.player_exit_reason.clone(),
        diagnostic_count: report.diagnostics.len(),
    }
}

pub(crate) fn report_path_for_desktop_export(report: &DesktopExportReport) -> String {
    Path::new(&report.package_dir)
        .join("reports")
        .join("desktop-export-report.json")
        .display()
        .to_string()
}

fn workspace_diagnostics_from_editor_diagnostics(
    diagnostics: &[EditorDiagnostic],
) -> UiWorkspaceDiagnosticsSummary {
    let mut summary = UiWorkspaceDiagnosticsSummary::default();
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

fn apply_report_panel_to_workspace(
    workspace: &mut ProjectAuthoringWorkspaceModel,
    report_panel: &ReportPanelModel,
) {
    workspace.report.report_count = report_panel.summary.report_count;
    workspace.report.evidence_count = report_panel.summary.evidence_count;
    workspace.report.next_action_count = report_panel.summary.next_action_count;
    workspace.report.diagnostics.info_count = report_panel.summary.info_count;
    workspace.report.diagnostics.warning_count = report_panel.summary.warning_count;
    workspace.report.diagnostics.error_count = report_panel.summary.error_count;

    if let Some(domain) = workspace
        .domains
        .iter_mut()
        .find(|domain| domain.kind == WorkspaceDomainKind::Report)
    {
        domain.status = if report_panel.summary.error_count > 0 {
            WorkspaceDomainStatus::Error
        } else if report_panel.summary.warning_count > 0 {
            WorkspaceDomainStatus::Warning
        } else if report_panel.summary.report_count == 0 {
            WorkspaceDomainStatus::Empty
        } else {
            WorkspaceDomainStatus::Ready
        };
        domain.item_count = report_panel.summary.report_count;
        domain.selected_id = report_panel.selected_report_id.clone();
        domain.diagnostics.info_count = report_panel.summary.info_count;
        domain.diagnostics.warning_count = report_panel.summary.warning_count;
        domain.diagnostics.error_count = report_panel.summary.error_count;
        domain.summary = format!(
            "providers={} reports={} evidence={} next_actions={}",
            report_panel.summary.provider_count,
            report_panel.summary.report_count,
            report_panel.summary.evidence_count,
            report_panel.summary.next_action_count
        );
    }
}

fn count_matching_files(dir: &Path, extensions_or_names: &[&str]) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .filter(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_string();
            extensions_or_names
                .iter()
                .any(|needle| file_name.ends_with(needle))
        })
        .count()
}

#[cfg(test)]
mod inspector_context_lock_tests {
    use super::*;
    use engine_runtime::components::Hierarchy;

    #[test]
    fn inspector_context_lock_keeps_typed_target_and_reports_unavailable_without_fallback() {
        let mut session = EditorSession::new();
        let mut world = World::new();
        for (id, name) in [("entity-a", "A"), ("entity-b", "B")] {
            world
                .try_spawn_entity(
                    EntityId::new(id),
                    name,
                    "test",
                    true,
                    Hierarchy {
                        parent_id: None,
                        sibling_order: 0,
                    },
                )
                .unwrap();
        }
        session.world = Some(world);
        session.selected_entity_source = Some(EntitySelectionSource::OpenedRuntimePackage);
        session.selected_entity_id = Some("entity-a".to_string());
        let anchor = session.inspector_context_anchor().expect("typed anchor");
        session.selected_entity_id = Some("entity-b".to_string());

        let locked = session.build_inspector_model_for_anchor(&anchor);
        assert_eq!(locked.selected_entity_id.as_deref(), Some("entity-a"));
        assert_eq!(locked.title, "A");
        assert_eq!(
            session
                .build_inspector_model()
                .selected_entity_id
                .as_deref(),
            Some("entity-b")
        );

        session
            .world
            .as_mut()
            .unwrap()
            .try_despawn_entity(&EntityId::new("entity-a"))
            .unwrap();
        let missing = session.build_inspector_model_for_anchor(&anchor);
        assert_eq!(missing.selected_entity_id.as_deref(), Some("entity-a"));
        assert_eq!(missing.title, "Locked target unavailable");

        session.world = None;
        let unavailable = session.build_inspector_model_for_anchor(&anchor);
        assert_eq!(unavailable.selected_entity_id.as_deref(), Some("entity-a"));
        assert_eq!(unavailable.title, "Locked target unavailable");
        assert!(unavailable.readonly);
    }
}
