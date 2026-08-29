use crate::command_system::{command_feedback_from_result, EditorCommandSystem};
use crate::composer::EditorUiModelComposer;
use crate::config::NativeEditorWindowConfig;
use crate::dialog::{
    default_project_dialog_initial_directory, HeadlessFolderDialogBackend,
    ProjectFolderDialogPurpose, ProjectFolderDialogRequest, ProjectFolderDialogResponse,
    ProjectLocationDialogService,
};
use crate::focus::{EditorFocusInputSystem, EditorMainFrame};
use crate::input_route::{ViewportInputGateway, ViewportInputRoute, ViewportInputRouteKind};
use crate::project_manager::ProjectManagerController;
use crate::transaction::EditorTransactionService;
use crate::viewport::ViewportHost;
use crate::{
    EditorPreferenceStore, EditorPreferencesDocument, WorkspaceLayoutStore,
    WorkspacePersistenceDiagnostic,
};
use editor_core::{
    CommandResult, CommandStatus, EditorAuthoringWorkspace, EditorPlayPreparationTicket,
    EditorPlayPreviewPackageReport, EditorPreviewPackageService, EditorSession,
    GameViewPresentDiagnostic, GameViewPresentReport, GameViewRuntimeFrame, InspectorContextAnchor,
    PreparedProjectOpen, ProjectManifest, ProjectOpenPreparation, ProjectOpenPreparationError,
    ProjectOpenPreparationPhase, ProjectPreviewEvidenceError, ProjectPreviewFrameEvidence,
    ProjectPreviewFrameReadback, ProjectPreviewFrameTicket, ProjectRuntimeNativeModuleBuildControl,
    ProjectRuntimeNativeModuleDiagnostic, ProjectRuntimePreparationTicket,
    ProjectRuntimeSourceKind, ProjectRuntimeTrustDecisionKind, ProjectRuntimeTrustInspection,
    ProjectRuntimeTrustModule, ProjectRuntimeTrustRequest, ProjectRuntimeTrustStatus,
    WorkspaceReport,
};
use editor_input::{
    AssetDragDropTarget, AssetDragInputState, AssetDragUpdate, EditorInputEvent, EditorInputRouter,
    EditorWidgetInteractionMachine,
};
use editor_ui_model::{
    ui_command_id_for_payload, DiagnosticSeverity, DiagnosticSource, EditorCatalogDiagnostic,
    EditorCatalogDiagnosticCode, EditorCommandFeedback, EditorDiagnostic, EditorLocaleChangeResult,
    EditorLocaleId, EditorLocalizationSnapshot, EditorUiMode, EditorUiModel,
    GatewayAccessRequestModel, ProjectOpenActivityModel, ProjectOpenActivityPhase, UiCommand,
    UiCommandPayload, UiCommandSource,
};
use editor_ui_renderer::{
    editor_workspace_rect, DockSplitAxis, DrawCommand, EditorWorkspaceDockingModule, HitTarget,
    LayoutNodeId, PanelId, PanelRegistry, RetainedEditorUiRenderer, UiDrawList, UiPoint,
    UiRendererConfig, WorkspaceDragWindowFacts, WorkspaceIntent, WorkspaceUpdate,
    WorkspaceWindowId, WorkspaceWindowPlacement,
};
use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct ProjectRuntimeTrustEnvironment {
    pub trust_module: ProjectRuntimeTrustModule,
    pub engine_sdk_root: PathBuf,
    pub editor_build_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedProjectRuntimeTrustRequest {
    pub project_root: PathBuf,
    pub trust_request: ProjectRuntimeTrustRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRuntimePreparationPhase {
    RevalidatingTrust,
    PreparingArtifact,
    LoadingModule,
}

pub struct PreparedProjectRuntime {
    pub identity: editor_core::ProjectNativeModuleIdentity,
    pub linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
}

pub trait ProjectRuntimePreparationAdapter: Send + Sync {
    fn prepare(
        &self,
        approved: ApprovedProjectRuntimeTrustRequest,
        control: ProjectRuntimeNativeModuleBuildControl,
        progress: &mut dyn FnMut(ProjectRuntimePreparationPhase),
    ) -> Result<PreparedProjectRuntime, ProjectRuntimeNativeModuleDiagnostic>;
}

pub trait ProjectOpenPreparationAdapter: Send + Sync {
    fn prepare(
        &self,
        project_root: &Path,
        progress: &mut dyn FnMut(ProjectOpenPreparationPhase),
        cancelled: &AtomicBool,
    ) -> Result<PreparedProjectOpen, ProjectOpenPreparationError>;
}

pub trait EditorPlayPreparationAdapter: Send + Sync {
    fn prepare(
        &self,
        ticket: &EditorPlayPreparationTicket,
        cancelled: &AtomicBool,
    ) -> Result<EditorPlayPreviewPackageReport, editor_core::EditorPlayPreparationError>;
}

struct DefaultEditorPlayPreparationAdapter;

impl EditorPlayPreparationAdapter for DefaultEditorPlayPreparationAdapter {
    fn prepare(
        &self,
        ticket: &EditorPlayPreparationTicket,
        cancelled: &AtomicBool,
    ) -> Result<EditorPlayPreviewPackageReport, editor_core::EditorPlayPreparationError> {
        if cancelled.load(Ordering::Acquire) {
            return Err(editor_core::EditorPlayPreparationError {
                code: "editor.play_preparation.cancelled".to_string(),
                message: "Editor Play preparation was cancelled.".to_string(),
            });
        }
        let report = EditorPreviewPackageService::prepare(ticket.request.clone());
        if cancelled.load(Ordering::Acquire) {
            Err(editor_core::EditorPlayPreparationError {
                code: "editor.play_preparation.cancelled".to_string(),
                message: "Editor Play preparation was cancelled.".to_string(),
            })
        } else {
            Ok(report)
        }
    }
}

struct DefaultProjectOpenPreparationAdapter;

impl ProjectOpenPreparationAdapter for DefaultProjectOpenPreparationAdapter {
    fn prepare(
        &self,
        project_root: &Path,
        progress: &mut dyn FnMut(ProjectOpenPreparationPhase),
        cancelled: &AtomicBool,
    ) -> Result<PreparedProjectOpen, ProjectOpenPreparationError> {
        ProjectOpenPreparation::prepare_cancellable(project_root, progress, || {
            cancelled.load(Ordering::Acquire)
        })
    }
}

enum ProjectRuntimePreparationEvent {
    Progress(ProjectRuntimePreparationPhase),
    Completed(Result<PreparedProjectRuntime, ProjectRuntimeNativeModuleDiagnostic>),
}

struct ProjectRuntimePreparationWorker {
    receiver: mpsc::Receiver<ProjectRuntimePreparationEvent>,
    join: Option<std::thread::JoinHandle<()>>,
    ticket: ProjectRuntimePreparationTicket,
    control: ProjectRuntimeNativeModuleBuildControl,
}

enum ProjectOpenPreparationEvent {
    Progress(ProjectOpenPreparationPhase),
    Completed(Result<PreparedProjectOpen, ProjectOpenPreparationError>),
}

struct ProjectOpenPreparationWorker {
    receiver: mpsc::Receiver<ProjectOpenPreparationEvent>,
    join: Option<std::thread::JoinHandle<()>>,
    command: UiCommand,
    activity: ProjectOpenActivityModel,
    started_at: Instant,
    cancelled: Arc<AtomicBool>,
}

struct EditorPlayPreparationWorker {
    receiver: mpsc::Receiver<
        Result<EditorPlayPreviewPackageReport, editor_core::EditorPlayPreparationError>,
    >,
    join: Option<std::thread::JoinHandle<()>>,
    command: UiCommand,
    ticket: EditorPlayPreparationTicket,
    started_at: Instant,
    cancelled: Arc<AtomicBool>,
}

struct PendingProjectRuntimeTrust {
    inspection: ProjectRuntimeTrustInspection,
    identity_changed: bool,
}

enum ProjectRuntimeTrustReview {
    Continue,
    Prompted,
    Rejected(CommandResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorApplicationReport {
    pub mode: EditorUiMode,
    pub frame_index: u64,
    pub model_revision: u64,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub panel_count: usize,
    pub command_count: usize,
    pub last_command_id: Option<String>,
    pub last_command_status: Option<CommandStatus>,
    pub active_panel_id: Option<String>,
    pub hovered_panel_id: Option<String>,
    pub hovered_hit_id: Option<String>,
    pub pressed_hit_id: Option<String>,
    pub last_feedback: Option<EditorCommandFeedback>,
    pub redraw_requested: bool,
    pub workspace: WorkspaceReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePointerCursor {
    Default,
    ColumnResize,
    RowResize,
}

pub struct NativeEditorApplication {
    config: NativeEditorWindowConfig,
    session: EditorSession,
    input_router: EditorInputRouter,
    widget_interaction: EditorWidgetInteractionMachine,
    viewport_host: ViewportHost,
    viewport_input_gateway: ViewportInputGateway,
    last_viewport_input_route: Option<ViewportInputRoute>,
    asset_drag_input: AssetDragInputState,
    main_frame: EditorMainFrame,
    workspace_docking: EditorWorkspaceDockingModule,
    workspace_layout_store: Option<WorkspaceLayoutStore>,
    workspace_persistence_diagnostics: Vec<WorkspacePersistenceDiagnostic>,
    editor_preference_store: Option<EditorPreferenceStore>,
    localization_snapshot: EditorLocalizationSnapshot,
    localization_diagnostic: Option<EditorCatalogDiagnostic>,
    workspace_menu_open: bool,
    language_menu_open: bool,
    toolbar_overflow_open: bool,
    workspace_panel_popup: Option<(String, String)>,
    inspector_context_lock: Option<InspectorContextAnchor>,
    focus_input: EditorFocusInputSystem,
    command_system: EditorCommandSystem,
    transaction_service: EditorTransactionService,
    project_manager: ProjectManagerController,
    project_dialog: Box<dyn ProjectLocationDialogService>,
    project_dialog_initial_directory: PathBuf,
    authoring_workspace: EditorAuthoringWorkspace,
    latest_model: EditorUiModel,
    latest_draw_list: UiDrawList,
    retained_ui_renderer: RetainedEditorUiRenderer,
    latest_surface_width: f32,
    latest_surface_height: f32,
    active_workspace_window_id: WorkspaceWindowId,
    workspace_drag_windows: Vec<WorkspaceDragWindowFacts>,
    workspace_screen_pointer: Option<UiPoint>,
    frame_index: u64,
    last_command_id: Option<String>,
    last_command_status: Option<CommandStatus>,
    last_feedback: Option<EditorCommandFeedback>,
    redraw_requested: bool,
    gateway_core: ai_tool_gateway::GatewayCore,
    gateway_client: ai_tool_gateway::GatewayOwnerThreadClient,
    gateway_dispatcher: ai_tool_gateway::GatewayOwnerThreadDispatcher,
    gateway_host_enabled: bool,
    editor_instance_id: String,
    gateway_discovery_root_override: Option<PathBuf>,
    gateway_host: Option<ai_tool_gateway::EditorGatewayHost>,
    gateway_host_attempted_binding: Option<ai_tool_gateway::EditorGatewayHostBinding>,
    gateway_host_error: Option<ai_tool_gateway::GatewayControlError>,
    gateway_access_page: usize,
    last_gateway_access_decision_receipt: Option<ai_tool_gateway::GatewayAccessDecisionReceipt>,
    last_gateway_requests_processed: usize,
    project_runtime_trust_environment: Option<ProjectRuntimeTrustEnvironment>,
    pending_project_runtime_trust: Option<PendingProjectRuntimeTrust>,
    approved_project_runtime_trust: Option<ApprovedProjectRuntimeTrustRequest>,
    project_runtime_preparation_adapter: Option<Arc<dyn ProjectRuntimePreparationAdapter>>,
    project_runtime_preparation_worker: Option<ProjectRuntimePreparationWorker>,
    project_open_preparation_worker: Option<ProjectOpenPreparationWorker>,
    project_open_preparation_adapter: Arc<dyn ProjectOpenPreparationAdapter>,
    editor_play_preparation_worker: Option<EditorPlayPreparationWorker>,
    editor_play_preparation_adapter: Arc<dyn EditorPlayPreparationAdapter>,
}

#[derive(Default)]
struct NativeEditorGatewayBuildOptions {
    host_enabled: bool,
    discovery_root_override: Option<PathBuf>,
    wake: Option<ai_tool_gateway::GatewayOwnerThreadWake>,
}

impl NativeEditorApplication {
    pub fn new(config: NativeEditorWindowConfig) -> Self {
        let session = EditorSession::with_linked_project_runtimes(
            crate::default_editor_linked_project_runtimes(),
        );
        Self::build(
            config,
            session,
            ProjectManagerController::default(),
            Box::<HeadlessFolderDialogBackend>::default(),
            default_project_dialog_initial_directory(),
            NativeEditorGatewayBuildOptions {
                host_enabled: true,
                ..Default::default()
            },
        )
    }

    pub fn with_session(config: NativeEditorWindowConfig, session: EditorSession) -> Self {
        Self::with_project_manager(
            config,
            session,
            ProjectManagerController::default(),
            Box::<HeadlessFolderDialogBackend>::default(),
        )
    }

    pub fn with_session_and_gateway_discovery_root(
        config: NativeEditorWindowConfig,
        session: EditorSession,
        discovery_root: PathBuf,
    ) -> Self {
        Self::build(
            config,
            session,
            ProjectManagerController::default(),
            Box::<HeadlessFolderDialogBackend>::default(),
            default_project_dialog_initial_directory(),
            NativeEditorGatewayBuildOptions {
                host_enabled: true,
                discovery_root_override: Some(discovery_root),
                wake: None,
            },
        )
    }

    pub fn shutdown_llm(&mut self) -> editor_core::LlmShutdownReceipt {
        self.session
            .shutdown_llm(editor_core::LLM_SESSION_SHUTDOWN_DEADLINE)
    }

    pub fn gateway_active_client_count(&self) -> usize {
        self.gateway_core.active_client_bindings().len()
    }

    pub fn with_project_manager(
        config: NativeEditorWindowConfig,
        session: EditorSession,
        project_manager: ProjectManagerController,
        project_dialog: Box<dyn ProjectLocationDialogService>,
    ) -> Self {
        Self::with_project_manager_and_dialog_initial_directory(
            config,
            session,
            project_manager,
            project_dialog,
            default_project_dialog_initial_directory(),
        )
    }

    pub fn with_project_manager_and_dialog_initial_directory(
        config: NativeEditorWindowConfig,
        session: EditorSession,
        project_manager: ProjectManagerController,
        project_dialog: Box<dyn ProjectLocationDialogService>,
        project_dialog_initial_directory: PathBuf,
    ) -> Self {
        Self::build(
            config,
            session,
            project_manager,
            project_dialog,
            project_dialog_initial_directory,
            NativeEditorGatewayBuildOptions::default(),
        )
    }

    pub(crate) fn with_project_manager_and_dialog_initial_directory_and_gateway(
        config: NativeEditorWindowConfig,
        session: EditorSession,
        project_manager: ProjectManagerController,
        project_dialog: Box<dyn ProjectLocationDialogService>,
        project_dialog_initial_directory: PathBuf,
        gateway_wake: Option<ai_tool_gateway::GatewayOwnerThreadWake>,
        gateway_discovery_root_override: Option<PathBuf>,
    ) -> Self {
        Self::build(
            config,
            session,
            project_manager,
            project_dialog,
            project_dialog_initial_directory,
            NativeEditorGatewayBuildOptions {
                host_enabled: true,
                discovery_root_override: gateway_discovery_root_override,
                wake: gateway_wake,
            },
        )
    }

    fn build(
        config: NativeEditorWindowConfig,
        mut session: EditorSession,
        mut project_manager: ProjectManagerController,
        project_dialog: Box<dyn ProjectLocationDialogService>,
        project_dialog_initial_directory: PathBuf,
        gateway_options: NativeEditorGatewayBuildOptions,
    ) -> Self {
        project_manager.load_recent_projects(&mut session);
        let initial_surface_width = config.width as f32;
        let initial_surface_height = config.height as f32;
        let model = EditorUiModelComposer::compose(&session);
        let mut authoring_workspace = EditorAuthoringWorkspace::new();
        authoring_workspace.refresh_from_model(&model);
        let workspace_docking = EditorWorkspaceDockingModule::standard_editor();
        let mut retained_ui_renderer = RetainedEditorUiRenderer::default();
        let workspace_snapshot = workspace_docking.snapshot(editor_workspace_rect(
            initial_surface_width,
            initial_surface_height,
        ));
        let draw_list = retained_ui_renderer.build_draw_list(
            &model,
            UiRendererConfig::new(initial_surface_width, initial_surface_height)
                .with_workspace_snapshot(workspace_snapshot),
        );
        let (gateway_client, gateway_dispatcher) = match gateway_options.wake {
            Some(wake) => ai_tool_gateway::gateway_owner_thread_channel_with_wake(wake),
            None => ai_tool_gateway::gateway_owner_thread_channel(),
        };
        let editor_instance_id = ai_tool_gateway::new_editor_instance_id();
        let mut application = Self {
            config,
            session,
            input_router: EditorInputRouter::new(),
            widget_interaction: EditorWidgetInteractionMachine::new(),
            viewport_host: ViewportHost::new(),
            viewport_input_gateway: ViewportInputGateway::new(),
            last_viewport_input_route: None,
            asset_drag_input: AssetDragInputState::default(),
            main_frame: EditorMainFrame::default(),
            workspace_docking,
            workspace_layout_store: None,
            workspace_persistence_diagnostics: Vec::new(),
            editor_preference_store: None,
            localization_snapshot: EditorLocalizationSnapshot::default(),
            localization_diagnostic: None,
            workspace_menu_open: false,
            language_menu_open: false,
            toolbar_overflow_open: false,
            workspace_panel_popup: None,
            inspector_context_lock: None,
            focus_input: EditorFocusInputSystem::default(),
            command_system: EditorCommandSystem::standard_editor(),
            transaction_service: EditorTransactionService::default(),
            project_manager,
            project_dialog,
            project_dialog_initial_directory,
            authoring_workspace,
            latest_model: model,
            latest_draw_list: draw_list,
            retained_ui_renderer,
            latest_surface_width: initial_surface_width,
            latest_surface_height: initial_surface_height,
            active_workspace_window_id: WorkspaceWindowId::main(),
            workspace_drag_windows: Vec::new(),
            workspace_screen_pointer: None,
            frame_index: 0,
            last_command_id: None,
            last_command_status: None,
            last_feedback: None,
            redraw_requested: true,
            gateway_core: ai_tool_gateway::GatewayCore::new_for_editor_instance(
                editor_instance_id.clone(),
            ),
            gateway_client,
            gateway_dispatcher,
            gateway_host_enabled: gateway_options.host_enabled,
            editor_instance_id,
            gateway_discovery_root_override: gateway_options.discovery_root_override,
            gateway_host: None,
            gateway_host_attempted_binding: None,
            gateway_host_error: None,
            gateway_access_page: 0,
            last_gateway_access_decision_receipt: None,
            last_gateway_requests_processed: 0,
            project_runtime_trust_environment: None,
            pending_project_runtime_trust: None,
            approved_project_runtime_trust: None,
            project_runtime_preparation_adapter: None,
            project_runtime_preparation_worker: None,
            project_open_preparation_worker: None,
            project_open_preparation_adapter: Arc::new(DefaultProjectOpenPreparationAdapter),
            editor_play_preparation_worker: None,
            editor_play_preparation_adapter: Arc::new(DefaultEditorPlayPreparationAdapter),
        };
        #[cfg(not(test))]
        if let Some(store) = crate::default_workspace_layout_store() {
            application.install_workspace_layout_store(store);
        }
        #[cfg(not(test))]
        if let Some(store) = crate::default_editor_preference_store() {
            application.install_editor_preference_store(store);
        }
        application.reconcile_gateway_host();
        application
    }

    pub fn frame(&mut self, width: f32, height: f32) -> NativeEditorApplicationReport {
        self.frame_workspace_window(
            &editor_ui_renderer::WorkspaceWindowId::main(),
            width,
            height,
        )
    }

    pub(crate) fn frame_workspace_window(
        &mut self,
        window_id: &editor_ui_renderer::WorkspaceWindowId,
        width: f32,
        height: f32,
    ) -> NativeEditorApplicationReport {
        self.latest_surface_width = width;
        self.latest_surface_height = height;
        self.reconcile_gateway_host();
        let gateway_operation_steps = self.gateway_core.pump_operations(&mut self.session, 1);
        self.last_gateway_requests_processed = self
            .gateway_dispatcher
            .pump(&mut self.gateway_core, &mut self.session);
        let asset_worker_changed = self.session.pump_asset_browser_refresh();
        let llm_worker_changed = self.session.pump_llm_patch_request();
        let project_runtime_preparation_changed = self.pump_project_runtime_preparation();
        let project_open_preparation_changed = self.pump_project_open_preparation();
        let editor_play_preparation_changed = self.pump_editor_play_preparation();
        self.latest_model = EditorUiModelComposer::compose(&self.session);
        self.sync_editor_play_activity();
        self.sync_project_editor_composition_actionability();
        self.apply_inspector_context_lock();
        self.sync_gateway_access_requests();
        self.sync_project_runtime_trust_prompt();
        self.sync_project_open_activity();
        self.latest_model.interaction_feedback = self.last_feedback.clone();
        self.authoring_workspace
            .refresh_from_model(&self.latest_model);
        let renderer_config = self.renderer_config_for_window(window_id, width, height);
        self.latest_draw_list = self
            .retained_ui_renderer
            .build_draw_list(&self.latest_model, renderer_config);
        self.sync_game_view_input_viewport();
        let visible_thumbnail_ids = image_texture_ids(&self.latest_draw_list);
        let thumbnail_requests_started = self
            .session
            .request_asset_thumbnail_ids(&visible_thumbnail_ids);
        let asset_work_pending = self.session.asset_thumbnail_summary().pending_count > 0
            || self.latest_model.asset_browser.index_status
                == editor_ui_model::AssetBrowserIndexStatus::Scanning;
        self.frame_index += 1;
        self.redraw_requested = asset_worker_changed
            || llm_worker_changed
            || project_runtime_preparation_changed
            || project_open_preparation_changed
            || editor_play_preparation_changed
            || gateway_operation_steps > 0
            || self.last_gateway_requests_processed > 0
            || self.session.has_active_llm_patch_request()
            || self.project_open_preparation_worker.is_some()
            || self.project_runtime_preparation_worker.is_some()
            || self.editor_play_preparation_worker.is_some()
            || thumbnail_requests_started > 0
            || asset_work_pending;
        self.report()
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn prepare_workspace_window_input(
        &mut self,
        window_id: &WorkspaceWindowId,
        width: f32,
        height: f32,
        screen_pointer: Option<UiPoint>,
        drag_windows: Vec<WorkspaceDragWindowFacts>,
    ) {
        self.active_workspace_window_id = window_id.clone();
        self.workspace_screen_pointer = screen_pointer;
        self.workspace_drag_windows = drag_windows;
        self.latest_surface_width = width;
        self.latest_surface_height = height;
        let renderer_config = self.renderer_config_for_window(window_id, width, height);
        self.latest_draw_list = self
            .retained_ui_renderer
            .build_draw_list(&self.latest_model, renderer_config);
        self.sync_game_view_input_viewport();
    }

    pub fn handle_input_event(&mut self, event: EditorInputEvent) -> NativeEditorApplicationReport {
        self.redraw_requested = true;
        if matches!(&event, EditorInputEvent::KeyDown { key } if key.eq_ignore_ascii_case("Escape"))
            && self.workspace_panel_popup.take().is_some()
        {
            self.redraw_requested = true;
            return self.report();
        }
        if matches!(event, EditorInputEvent::FocusLost)
            && self.workspace_panel_popup.take().is_some()
        {
            self.redraw_requested = true;
        }
        if matches!(event, EditorInputEvent::FocusLost) {
            self.focus_input.observe_event(
                &event,
                &self.latest_draw_list,
                self.retained_ui_renderer.tree(),
            );
        }
        let interaction_update = self
            .retained_ui_renderer
            .tree()
            .map(|tree| self.widget_interaction.handle_event(&event, tree))
            .unwrap_or_default();
        if !interaction_update.dirty_widget_ids.is_empty() {
            self.redraw_requested = true;
        }
        if self.consume_workspace_input(&event) {
            return self.report();
        }
        if let EditorInputEvent::KeyDown { key } = &event {
            if matches!(key.as_str(), "Tab" | "Shift+Tab") {
                if let Some(tree) = self.retained_ui_renderer.tree() {
                    let focused = self.focus_input.focus_next(tree, key == "Shift+Tab");
                    self.widget_interaction.set_keyboard_focus(focused);
                    self.redraw_requested = true;
                }
                return self.report();
            }
        }
        if let EditorInputEvent::MouseWheel { delta } = &event {
            if let Some((x, y)) = self.focus_input.last_pointer_position {
                if self
                    .retained_ui_renderer
                    .scroll_at(UiPoint { x, y }, -*delta * 24.0)
                    .is_some()
                {
                    self.redraw_requested = true;
                    return self.report();
                }
            }
        }
        if self.consume_input_mapping_capture(&event) {
            return self.report();
        }
        if self.consume_ai_panel_input(&event) {
            return self.report();
        }
        if self.consume_asset_drag_input(&event) {
            return self.report();
        }
        if self.consume_asset_browser_input(&event) {
            return self.report();
        }
        let game_view_input_consumed = self.try_route_production_game_view_input(&event);
        if game_view_input_consumed && !interaction_update.handled {
            return self.report();
        }
        self.focus_input.observe_event(
            &event,
            &self.latest_draw_list,
            self.retained_ui_renderer.tree(),
        );
        if let Some(widget_id) = interaction_update.activation.clone() {
            if self.consume_local_control_activation(&widget_id) {
                return self.report();
            }
        }
        if !interaction_update.handled {
            if let EditorInputEvent::PointerDown { x, y, .. } = &event {
                if let Some(region) =
                    editor_ui_renderer::hit_test(&self.latest_draw_list, UiPoint { x: *x, y: *y })
                        .cloned()
                {
                    if let HitTarget::DockTab { panel_id } = &region.target {
                        self.workspace_panel_popup = None;
                        let update = PanelId::new(panel_id.clone())
                            .map(|panel_id| {
                                self.workspace_docking
                                    .update(WorkspaceIntent::ActivatePanel { panel_id })
                            })
                            .filter(|update| update.diagnostics.is_empty());
                        if update.is_some_and(|update| update.changed) {
                            self.redraw_requested = true;
                            return self.report();
                        }
                    } else if matches!(&region.target, HitTarget::WorkspaceWindowMenu) {
                        self.workspace_panel_popup = None;
                        self.language_menu_open = false;
                        self.workspace_menu_open = !self.workspace_menu_open;
                        self.redraw_requested = true;
                        return self.report();
                    } else if matches!(&region.target, HitTarget::EditorLanguageMenu) {
                        self.workspace_panel_popup = None;
                        self.workspace_menu_open = false;
                        self.language_menu_open = !self.language_menu_open;
                        self.redraw_requested = true;
                        return self.report();
                    } else if let HitTarget::SetEditorLocale { locale } = &region.target {
                        self.language_menu_open = false;
                        self.change_editor_locale(locale.clone());
                        self.redraw_requested = true;
                        return self.report();
                    } else if let HitTarget::WorkspacePanelVisibility { panel_id, visible } =
                        &region.target
                    {
                        if *visible {
                            self.close_workspace_panel(panel_id);
                        } else {
                            self.show_workspace_panel(panel_id);
                        }
                        self.workspace_menu_open = false;
                        self.redraw_requested = true;
                        return self.report();
                    } else if matches!(&region.target, HitTarget::WorkspaceResetLayout) {
                        self.reset_workspace_layout();
                        self.workspace_menu_open = false;
                        self.redraw_requested = true;
                        return self.report();
                    } else if matches!(&region.target, HitTarget::ToolbarOverflow) {
                        self.workspace_panel_popup = None;
                        self.toolbar_overflow_open = !self.toolbar_overflow_open;
                        self.redraw_requested = true;
                        return self.report();
                    } else if let HitTarget::WorkspacePanelMore { stack_id, panel_id } =
                        &region.target
                    {
                        let next = (stack_id.clone(), panel_id.clone());
                        if self.workspace_panel_popup.as_ref() == Some(&next) {
                            self.workspace_panel_popup = None;
                        } else {
                            self.workspace_panel_popup = Some(next);
                        }
                        self.redraw_requested = true;
                        return self.report();
                    } else if let HitTarget::WorkspacePanelClose { panel_id, .. } = &region.target {
                        if !region.enabled {
                            return self.report();
                        }
                        self.workspace_panel_popup = None;
                        self.close_workspace_panel(panel_id);
                        self.redraw_requested = true;
                        return self.report();
                    } else if let HitTarget::WorkspacePanelLock {
                        panel_id, locked, ..
                    } = &region.target
                    {
                        if panel_id == "inspector" && region.enabled {
                            self.inspector_context_lock = if *locked {
                                None
                            } else {
                                self.session.inspector_context_anchor()
                            };
                            self.workspace_panel_popup = None;
                            self.latest_model = EditorUiModelComposer::compose(&self.session);
                            self.apply_inspector_context_lock();
                            self.redraw_requested = true;
                        }
                        return self.report();
                    } else if self.workspace_panel_popup.take().is_some() {
                        self.redraw_requested = true;
                    } else if let HitTarget::InspectorField { field_id } = &region.target {
                        if self
                            .authoring_workspace
                            .begin_property_edit(field_id)
                            .is_ok()
                        {
                            self.redraw_requested = true;
                        }
                    }
                } else if self.workspace_panel_popup.take().is_some() {
                    self.redraw_requested = true;
                }
            }
        }
        let route = if let Some(widget_id) = interaction_update
            .disabled
            .as_ref()
            .or(interaction_update.activation.as_ref())
        {
            let tree = self
                .retained_ui_renderer
                .tree()
                .expect("interaction update requires retained tree");
            self.input_router.route_widget_activation(widget_id, tree)
        } else if interaction_update.handled {
            return self.report();
        } else if let Some(tree) = self.retained_ui_renderer.tree() {
            self.input_router
                .route_widget(event, tree, self.focus_input.pointer_capture.as_ref())
        } else {
            self.input_router.route(event, &self.latest_draw_list)
        };
        if let Some(feedback) = route.disabled_feedback {
            self.last_command_id = Some(feedback.command_id.clone());
            self.last_command_status = None;
            self.last_feedback = Some(feedback);
            self.latest_model.interaction_feedback = self.last_feedback.clone();
            self.redraw_requested = true;
            return self.report();
        }
        if let Some(command) = route.command {
            self.dispatch_project_launcher_command_or_dispatch(command);
        }
        self.report()
    }

    fn try_route_production_game_view_input(&mut self, event: &EditorInputEvent) -> bool {
        if matches!(event, EditorInputEvent::FocusLost) {
            let _ = self.viewport_host.focus_game(false);
            self.session.cancel_active_game_view_input();
            return false;
        }
        let pointer = match event {
            EditorInputEvent::PointerDown { x, y, .. }
            | EditorInputEvent::PointerUp { x, y, .. }
            | EditorInputEvent::PointerMove { x, y } => Some(UiPoint { x: *x, y: *y }),
            _ => None,
        };
        if let Some(pointer) = pointer {
            let Some(viewport) = self.viewport_host.game_viewport() else {
                return false;
            };
            if !viewport.rect.contains(pointer) {
                return false;
            }
        } else if !self
            .viewport_host
            .game_viewport()
            .is_some_and(|viewport| viewport.focused)
        {
            return false;
        }
        let ui_hit = pointer.is_some_and(|pointer| {
            editor_ui_renderer::hit_test(&self.latest_draw_list, pointer)
                .is_some_and(|region| !matches!(region.target, HitTarget::Viewport))
        });
        let route = self.viewport_input_gateway.route_editor_input(
            event.clone(),
            ui_hit,
            &mut self.viewport_host,
        );
        let consumed = matches!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
        if let Some(runtime_input) = route.runtime_input_frame.clone() {
            self.session.route_active_game_view_aui_input(runtime_input);
            self.redraw_requested = true;
        }
        self.last_viewport_input_route = Some(route);
        consumed
    }

    fn sync_game_view_input_viewport(&mut self) {
        let Some(frame) = self.session.last_game_view_runtime_frame() else {
            self.viewport_host.clear_game_viewport();
            return;
        };
        let Some(rect) = self.latest_draw_list.commands.iter().find_map(|command| {
            viewport_texture_content_rect(command, &frame.texture_id, &frame.target_id, None)
        }) else {
            self.viewport_host.clear_game_viewport();
            return;
        };
        let canvas_references = self
            .session
            .active_game_view_aui_action_targets()
            .iter()
            .map(|target| {
                (
                    target.canvas_id.clone(),
                    (target.reference_width, target.reference_height),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(canvas_id, (width, height))| {
                engine_runtime::game_view_presentation::CanvasReferenceFact::new(
                    canvas_id, width, height,
                )
            })
            .collect();
        if self.viewport_host.game_viewport().is_none() {
            if self
                .viewport_host
                .register_game_viewport("editor-game-view", rect)
                .is_err()
            {
                return;
            }
        } else if self.viewport_host.update_game_rect(rect).is_err() {
            self.viewport_host.clear_game_viewport();
            return;
        }
        if self
            .viewport_host
            .update_game_presentation_with_canvases(
                frame.width,
                frame.height,
                frame.presentation_scale_policy,
                canvas_references,
            )
            .is_err()
        {
            self.viewport_host.clear_game_viewport();
        }
    }

    #[cfg(test)]
    pub(crate) fn configure_game_view_input_viewport_for_test(
        &mut self,
        rect: editor_ui_renderer::UiRect,
        runtime_width: u32,
        runtime_height: u32,
    ) {
        self.viewport_host.clear_game_viewport();
        self.viewport_host
            .register_game_viewport("editor-game-view", rect)
            .expect("test GameView rect must be valid");
        self.viewport_host
            .update_game_runtime_extent(runtime_width, runtime_height)
            .expect("test GameView extent must be valid");
    }

    #[cfg(test)]
    pub(crate) fn configure_game_view_input_presentation_for_test(
        &mut self,
        rect: editor_ui_renderer::UiRect,
        runtime_width: u32,
        runtime_height: u32,
        canvas_references: Vec<engine_runtime::game_view_presentation::CanvasReferenceFact>,
    ) {
        self.viewport_host.clear_game_viewport();
        self.viewport_host
            .register_game_viewport("editor-game-view", rect)
            .expect("test GameView rect must be valid");
        self.viewport_host
            .update_game_presentation_with_canvases(
                runtime_width,
                runtime_height,
                engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
                canvas_references,
            )
            .expect("test GameView presentation must be valid");
    }

    pub(crate) fn last_viewport_input_route(&self) -> Option<&ViewportInputRoute> {
        self.last_viewport_input_route.as_ref()
    }

    fn consume_workspace_input(&mut self, event: &EditorInputEvent) -> bool {
        if self.workspace_docking.active_panel_drag_id().is_some() {
            match event {
                EditorInputEvent::PointerMove { x, y } => {
                    self.focus_input.last_pointer_position = Some((*x, *y));
                    let intent = if let Some(screen_pointer) = self.workspace_screen_pointer {
                        WorkspaceIntent::UpdatePanelDragAcrossWindows {
                            screen_pointer,
                            windows: self.workspace_drag_windows.clone(),
                        }
                    } else {
                        WorkspaceIntent::UpdatePanelDrag {
                            pointer: UiPoint { x: *x, y: *y },
                            workspace_rect: editor_workspace_rect(
                                self.latest_surface_width,
                                self.latest_surface_height,
                            ),
                        }
                    };
                    self.workspace_docking.update(intent);
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::PointerUp {
                    x,
                    y,
                    button: editor_input::PointerButton::Primary,
                } => {
                    if self.workspace_docking.drag_requires_native_proxy() {
                        let window_id = self.next_floating_workspace_window_id();
                        let screen = self
                            .workspace_screen_pointer
                            .unwrap_or(UiPoint { x: *x, y: *y });
                        self.workspace_docking
                            .update(WorkspaceIntent::CommitPanelDragToFloating {
                                window_id,
                                placement: WorkspaceWindowPlacement {
                                    x: screen.x - 320.0,
                                    y: screen.y - 24.0,
                                    ..WorkspaceWindowPlacement::default()
                                },
                            });
                    } else {
                        self.workspace_docking
                            .update(WorkspaceIntent::CommitPanelDrag);
                    }
                    self.persist_workspace_layout();
                    self.focus_input.observe_event(
                        event,
                        &self.latest_draw_list,
                        self.retained_ui_renderer.tree(),
                    );
                    self.focus_input.last_pointer_position = Some((*x, *y));
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::FocusLost => {
                    self.workspace_docking
                        .update(WorkspaceIntent::CancelPanelDrag);
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::KeyDown { key } if key.eq_ignore_ascii_case("Escape") => {
                    self.workspace_docking
                        .update(WorkspaceIntent::CancelPanelDrag);
                    self.release_workspace_pointer_capture();
                    self.redraw_requested = true;
                    return true;
                }
                _ => return false,
            }
        }

        if self.workspace_docking.active_resize_node_id().is_some() {
            match event {
                EditorInputEvent::PointerMove { x, y } => {
                    self.focus_input.last_pointer_position = Some((*x, *y));
                    self.workspace_docking
                        .update(WorkspaceIntent::UpdateSplitterResize {
                            pointer: UiPoint { x: *x, y: *y },
                        });
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::PointerUp {
                    x,
                    y,
                    button: editor_input::PointerButton::Primary,
                } => {
                    self.workspace_docking
                        .update(WorkspaceIntent::CommitSplitterResize);
                    self.persist_workspace_layout();
                    self.focus_input.observe_event(
                        event,
                        &self.latest_draw_list,
                        self.retained_ui_renderer.tree(),
                    );
                    self.focus_input.last_pointer_position = Some((*x, *y));
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::FocusLost => {
                    self.workspace_docking
                        .update(WorkspaceIntent::CancelSplitterResize);
                    self.redraw_requested = true;
                    return true;
                }
                EditorInputEvent::KeyDown { key } if key.eq_ignore_ascii_case("Escape") => {
                    self.workspace_docking
                        .update(WorkspaceIntent::CancelSplitterResize);
                    self.release_workspace_pointer_capture();
                    self.redraw_requested = true;
                    return true;
                }
                _ => return false,
            }
        }

        let EditorInputEvent::PointerDown {
            x,
            y,
            button: editor_input::PointerButton::Primary,
        } = event
        else {
            return false;
        };
        let Some(tree) = self.retained_ui_renderer.tree() else {
            return false;
        };
        let Some(pick) = editor_ui_renderer::pick_widget(tree, UiPoint { x: *x, y: *y }, None)
        else {
            return false;
        };
        let Some(target) = tree
            .node(&pick.target)
            .and_then(|node| node.binding.as_ref())
            .map(|binding| &binding.target)
        else {
            return false;
        };
        let target = target.clone();
        self.focus_input.observe_event(
            event,
            &self.latest_draw_list,
            self.retained_ui_renderer.tree(),
        );
        let workspace_rect =
            editor_workspace_rect(self.latest_surface_width, self.latest_surface_height);
        let update = match target {
            HitTarget::WorkspaceSplitter { node_id } => {
                let Some(node_id) = LayoutNodeId::new(node_id) else {
                    self.release_workspace_pointer_capture();
                    return false;
                };
                self.workspace_docking
                    .update(WorkspaceIntent::BeginSplitterResize {
                        node_id,
                        pointer: UiPoint { x: *x, y: *y },
                        workspace_rect,
                    })
            }
            HitTarget::DockTab { panel_id } => {
                self.workspace_panel_popup = None;
                let Some(panel_id) = PanelId::new(panel_id) else {
                    self.release_workspace_pointer_capture();
                    return false;
                };
                let update =
                    self.workspace_docking
                        .update(WorkspaceIntent::BeginPanelDragInWindow {
                            panel_id: panel_id.clone(),
                            source_window_id: self.active_workspace_window_id.clone(),
                            pointer: UiPoint { x: *x, y: *y },
                            workspace_rect,
                        });
                if update.diagnostics.is_empty() {
                    self.workspace_docking
                        .update(WorkspaceIntent::ActivatePanel { panel_id });
                }
                update
            }
            _ => {
                self.release_workspace_pointer_capture();
                return false;
            }
        };
        if !update.diagnostics.is_empty() {
            self.release_workspace_pointer_capture();
            return false;
        }
        self.redraw_requested = true;
        true
    }

    fn next_floating_workspace_window_id(&self) -> WorkspaceWindowId {
        let topology = self.workspace_docking.topology();
        let mut suffix = self
            .workspace_docking
            .snapshot(editor_workspace_rect(
                self.latest_surface_width,
                self.latest_surface_height,
            ))
            .layout_revision
            .saturating_add(1);
        loop {
            let candidate =
                WorkspaceWindowId::new(format!("floating-{suffix}")).expect("non-empty window id");
            if topology
                .floating_roots
                .iter()
                .all(|root| root.window_id != candidate)
            {
                return candidate;
            }
            suffix = suffix.saturating_add(1);
        }
    }

    fn release_workspace_pointer_capture(&mut self) {
        self.focus_input.pointer_capture = None;
        self.focus_input.mouse_captured = false;
        self.focus_input.pressed_hit_id = None;
    }

    fn consume_local_control_activation(
        &mut self,
        widget_id: &editor_ui_renderer::WidgetId,
    ) -> bool {
        let binding = self
            .retained_ui_renderer
            .tree()
            .and_then(|tree| tree.node(widget_id))
            .and_then(|node| node.binding.as_ref())
            .map(|binding| (binding.target.clone(), binding.command_id.clone()));
        match binding {
            Some((HitTarget::ToolbarOverflow, command_id)) => {
                self.workspace_panel_popup = None;
                self.toolbar_overflow_open = if command_id == "close_toolbar_overflow" {
                    false
                } else {
                    !self.toolbar_overflow_open
                };
                self.redraw_requested = true;
                true
            }
            Some((HitTarget::WorkspacePanelMore { stack_id, panel_id }, _)) => {
                let next = (stack_id, panel_id);
                self.workspace_panel_popup =
                    (self.workspace_panel_popup.as_ref() != Some(&next)).then_some(next);
                self.redraw_requested = true;
                true
            }
            Some((HitTarget::WorkspacePanelClose { panel_id, .. }, _)) => {
                self.workspace_panel_popup = None;
                self.close_workspace_panel(&panel_id);
                self.redraw_requested = true;
                true
            }
            Some((
                HitTarget::WorkspacePanelLock {
                    panel_id, locked, ..
                },
                _,
            )) if panel_id == "inspector" => {
                self.inspector_context_lock = if locked {
                    None
                } else {
                    self.session.inspector_context_anchor()
                };
                self.workspace_panel_popup = None;
                self.latest_model = EditorUiModelComposer::compose(&self.session);
                self.apply_inspector_context_lock();
                self.redraw_requested = true;
                true
            }
            _ => false,
        }
    }

    fn consume_asset_browser_input(&mut self, event: &EditorInputEvent) -> bool {
        let EditorInputEvent::KeyDown { key } = event else {
            return false;
        };
        if self.latest_model.asset_browser.picker.is_some() {
            match key.as_str() {
                "Escape" => {
                    self.dispatch_asset_browser_payload(UiCommandPayload::CancelAssetPick);
                    return true;
                }
                "Enter" => {
                    self.dispatch_asset_browser_payload(UiCommandPayload::ConfirmAssetPick);
                    return true;
                }
                _ => {}
            }
        }
        if matches!(
            self.focus_input
                .focused_target(self.retained_ui_renderer.tree()),
            Some(HitTarget::AssetBrowserSearch)
        ) {
            let mut search = self.latest_model.asset_browser.query.search_text.clone();
            match key.as_str() {
                "Backspace" => {
                    search.pop();
                }
                "Escape" => {
                    search.clear();
                    self.focus_input.keyboard_focus = None;
                }
                "Space" => search.push(' '),
                value if value.chars().count() == 1 => search.push_str(value),
                _ => return false,
            }
            self.dispatch_asset_browser_payload(UiCommandPayload::SetAssetBrowserSearch {
                search_text: search,
            });
            return true;
        }

        if self.focus_input.active_panel_id.as_deref() != Some("asset_browser") {
            return false;
        }
        let entries = self
            .latest_model
            .asset_browser
            .entries
            .iter()
            .filter(|entry| entry.role != editor_ui_model::AssetEntryRole::Folder)
            .collect::<Vec<_>>();
        match key.as_str() {
            "ArrowDown" | "Down" | "ArrowRight" | "Right" => {
                let current = self
                    .latest_model
                    .asset_browser
                    .selection
                    .primary_entry_key
                    .as_ref()
                    .and_then(|key| entries.iter().position(|entry| &entry.entry_key == key));
                let next =
                    current.map_or(0, |index| (index + 1).min(entries.len().saturating_sub(1)));
                let Some(entry) = entries.get(next) else {
                    return true;
                };
                self.dispatch_asset_browser_payload(UiCommandPayload::SelectAssetBrowserEntry {
                    entry_key: entry.entry_key.clone(),
                    additive: false,
                    range: false,
                });
                true
            }
            "ArrowUp" | "Up" | "ArrowLeft" | "Left" => {
                let current = self
                    .latest_model
                    .asset_browser
                    .selection
                    .primary_entry_key
                    .as_ref()
                    .and_then(|key| entries.iter().position(|entry| &entry.entry_key == key))
                    .unwrap_or(0);
                let Some(entry) = entries.get(current.saturating_sub(1)) else {
                    return true;
                };
                self.dispatch_asset_browser_payload(UiCommandPayload::SelectAssetBrowserEntry {
                    entry_key: entry.entry_key.clone(),
                    additive: false,
                    range: false,
                });
                true
            }
            "Enter" => {
                let Some(entry_key) = self
                    .latest_model
                    .asset_browser
                    .selection
                    .primary_entry_key
                    .clone()
                else {
                    return true;
                };
                self.dispatch_asset_browser_payload(UiCommandPayload::OpenAssetBrowserEntry {
                    entry_key,
                });
                true
            }
            _ => false,
        }
    }

    fn consume_ai_panel_input(&mut self, event: &EditorInputEvent) -> bool {
        let EditorInputEvent::KeyDown { key } = event else {
            return false;
        };
        if !matches!(
            self.focus_input
                .focused_target(self.retained_ui_renderer.tree()),
            Some(HitTarget::AiPromptField)
        ) {
            return false;
        }
        if self.latest_model.ai_panel.busy {
            if key == "Escape" {
                self.dispatch_ai_panel_payload(UiCommandPayload::CancelLlmPatchRequest);
                return true;
            }
            return false;
        }

        let mut prompt = self.latest_model.ai_panel.prompt_draft.clone();
        match key.as_str() {
            "Enter" => {
                if !prompt.trim().is_empty() {
                    self.dispatch_ai_panel_payload(
                        UiCommandPayload::GenerateProjectPatchFromPrompt { prompt },
                    );
                }
            }
            "Backspace" => {
                prompt.pop();
                self.dispatch_ai_panel_payload(UiCommandPayload::SetAiPromptDraft { prompt });
            }
            "Escape" => {
                self.focus_input.keyboard_focus = None;
                self.dispatch_ai_panel_payload(UiCommandPayload::SetAiPromptDraft {
                    prompt: String::new(),
                });
            }
            "Space" => {
                prompt.push(' ');
                self.dispatch_ai_panel_payload(UiCommandPayload::SetAiPromptDraft { prompt });
            }
            value if value.chars().count() == 1 => {
                prompt.push_str(value);
                self.dispatch_ai_panel_payload(UiCommandPayload::SetAiPromptDraft { prompt });
            }
            _ => return false,
        }
        true
    }

    fn consume_asset_drag_input(&mut self, event: &EditorInputEvent) -> bool {
        match self
            .asset_drag_input
            .handle_event(event, &self.latest_draw_list)
        {
            AssetDragUpdate::None | AssetDragUpdate::Armed { .. } => false,
            AssetDragUpdate::Started { .. } | AssetDragUpdate::Hovering { .. } => {
                self.redraw_requested = true;
                true
            }
            AssetDragUpdate::Cancelled { was_dragging } => {
                self.redraw_requested = was_dragging;
                was_dragging
            }
            AssetDragUpdate::Dropped { entry_key, target } => {
                match target {
                    AssetDragDropTarget::Scene => {
                        let entry = self
                            .latest_model
                            .asset_browser
                            .entries
                            .iter()
                            .find(|entry| entry.entry_key == entry_key)
                            .cloned();
                        let Some(entry) = entry else {
                            self.record_asset_drag_rejection(
                                "asset_drag.entry_missing",
                                "Dragged asset is no longer present in the cached browser view.",
                            );
                            return true;
                        };
                        let payload = editor_core::AssetBrowserService::drag_payload(&[entry]);
                        let Some(asset_ref) = payload.asset_refs.first() else {
                            self.record_asset_drag_rejection(
                                "asset_drag.identity_required",
                                "Source files and folders cannot be dropped into Scene as AssetRef values.",
                            );
                            return true;
                        };
                        let placement =
                            match editor_core::AssetBrowserService::placement_request_from_reference(
                                asset_ref,
                                self.latest_model.hierarchy.selected_entity_id.clone(),
                                None,
                                editor_ui_model::AssetPlacementMode::UnderSelectedOrRoot,
                            ) {
                                Ok(placement) => placement,
                                Err(diagnostic) => {
                                    self.record_asset_drag_rejection(
                                        &diagnostic.code,
                                        &diagnostic.message,
                                    );
                                    return true;
                                }
                            };
                        self.dispatch_asset_browser_payload(
                            UiCommandPayload::PlaceAssetIntoScene {
                                asset_id: placement.asset_id,
                                asset_type: placement.asset_type,
                                asset_guid: placement.asset_guid,
                                target_parent_id: placement.target_parent_id,
                                local_position: placement.local_position.map(|position| {
                                    editor_ui_model::Vec3 {
                                        x: position.x,
                                        y: position.y,
                                        z: position.z,
                                    }
                                }),
                                placement_mode: placement.placement_mode,
                            },
                        );
                    }
                    AssetDragDropTarget::InspectorField { field_id } => {
                        self.dispatch_asset_browser_payload(
                            UiCommandPayload::DropAssetOnInspectorField {
                                entry_key,
                                field_id,
                            },
                        );
                    }
                }
                self.redraw_requested = true;
                true
            }
        }
    }

    fn record_asset_drag_rejection(&mut self, code: &str, message: &str) {
        self.last_command_id = Some(code.to_string());
        self.last_command_status = Some(CommandStatus::Rejected);
        self.last_feedback = Some(EditorCommandFeedback {
            command_id: code.to_string(),
            status: editor_ui_model::EditorCommandFeedbackStatus::Rejected,
            diagnostic_code: Some(code.to_string()),
            message: message.to_string(),
            reason: Some(message.to_string()),
            source: UiCommandSource::ProjectBrowser,
        });
        self.redraw_requested = true;
    }

    fn record_project_dialog_unavailable(&mut self, command: &UiCommand, diagnostic: String) {
        self.last_command_id = Some(command.command_id.clone());
        self.last_command_status = Some(CommandStatus::Rejected);
        self.last_feedback = Some(EditorCommandFeedback {
            command_id: command.command_id.clone(),
            status: editor_ui_model::EditorCommandFeedbackStatus::Rejected,
            diagnostic_code: Some(command.command_id.clone()),
            message: diagnostic.clone(),
            reason: Some(diagnostic),
            source: command.source.clone(),
        });
        self.latest_model.interaction_feedback = self.last_feedback.clone();
        let renderer_config =
            self.renderer_config(self.config.width as f32, self.config.height as f32);
        self.latest_draw_list = self
            .retained_ui_renderer
            .build_draw_list(&self.latest_model, renderer_config);
        self.redraw_requested = true;
    }

    fn dispatch_asset_browser_payload(&mut self, payload: UiCommandPayload) -> CommandResult {
        self.dispatch_command(UiCommand {
            command_id: ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::ProjectBrowser,
            request_id: format!("asset-browser-{}", self.frame_index + 1),
            payload,
        })
    }

    fn dispatch_ai_panel_payload(&mut self, payload: UiCommandPayload) -> CommandResult {
        self.dispatch_command(UiCommand {
            command_id: ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::AiAssistant,
            request_id: format!("ai-panel-{}", self.frame_index + 1),
            payload,
        })
    }

    fn consume_input_mapping_capture(&mut self, event: &EditorInputEvent) -> bool {
        let mapping = &self.latest_model.input_mapping_authoring;
        let Some(path) = mapping.selected_path.clone() else {
            return false;
        };
        let Some(binding_id) = mapping.capture_binding_id.clone() else {
            return false;
        };
        let payload = match event {
            EditorInputEvent::KeyDown { key } if key.eq_ignore_ascii_case("Escape") => {
                Some(UiCommandPayload::CancelInputBindingCapture { path })
            }
            EditorInputEvent::KeyDown { key } => {
                Some(UiCommandPayload::CommitCapturedInputBinding {
                    path,
                    binding_id,
                    device_path: format!("keyboard/{key}"),
                })
            }
            EditorInputEvent::PointerDown { button, .. } => {
                let button = match button {
                    editor_input::PointerButton::Primary => "Left",
                    editor_input::PointerButton::Secondary => "Right",
                    editor_input::PointerButton::Middle => "Middle",
                };
                Some(UiCommandPayload::CommitCapturedInputBinding {
                    path,
                    binding_id,
                    device_path: format!("mouse/{button}"),
                })
            }
            EditorInputEvent::MouseWheel { .. } => {
                Some(UiCommandPayload::CommitCapturedInputBinding {
                    path,
                    binding_id,
                    device_path: "mouse/Wheel".to_string(),
                })
            }
            EditorInputEvent::PointerMove { .. } if mapping.capture_accepts_pointer_position => {
                Some(UiCommandPayload::CommitCapturedInputBinding {
                    path,
                    binding_id,
                    device_path: "mouse/Position".to_string(),
                })
            }
            EditorInputEvent::FocusLost => {
                Some(UiCommandPayload::CancelInputBindingCapture { path })
            }
            EditorInputEvent::PointerUp { .. }
            | EditorInputEvent::PointerMove { .. }
            | EditorInputEvent::KeyUp { .. } => None,
        };
        if let Some(payload) = payload {
            let command_id = ui_command_id_for_payload(&payload).to_string();
            self.dispatch_command(UiCommand {
                command_id,
                source: UiCommandSource::Unknown,
                request_id: format!("input-capture-{}", self.frame_index + 1),
                payload,
            });
        }
        true
    }

    pub fn handle_shortcut(&mut self, shortcut: &str) -> NativeEditorApplicationReport {
        if let Some(command) = self.command_system.shortcut_command(shortcut) {
            self.dispatch_command(command);
        }
        self.report()
    }

    pub fn handle_text_input(&mut self, text: &str) -> bool {
        let target = self
            .focus_input
            .focused_target(self.retained_ui_renderer.tree())
            .cloned();
        if !self
            .focus_input
            .consume_text_input(text, self.retained_ui_renderer.tree())
        {
            return false;
        }
        match target {
            Some(HitTarget::InspectorField { .. }) => {
                self.authoring_workspace.input_property_text(text);
            }
            Some(HitTarget::AssetBrowserSearch) => {
                let mut search = self.latest_model.asset_browser.query.search_text.clone();
                search.push_str(text);
                self.dispatch_asset_browser_payload(UiCommandPayload::SetAssetBrowserSearch {
                    search_text: search,
                });
            }
            Some(HitTarget::AiPromptField) => {
                let mut prompt = self.latest_model.ai_panel.prompt_draft.clone();
                prompt.push_str(text);
                self.dispatch_ai_panel_payload(UiCommandPayload::SetAiPromptDraft { prompt });
            }
            _ => return false,
        }
        self.redraw_requested = true;
        true
    }

    pub fn replace_focused_property_text(&mut self, text: impl Into<String>) -> bool {
        if !matches!(
            self.focus_input
                .focused_target(self.retained_ui_renderer.tree()),
            Some(HitTarget::InspectorField { .. })
        ) {
            return false;
        }
        self.authoring_workspace.replace_property_text(text);
        self.redraw_requested = true;
        true
    }

    pub fn commit_focused_property_edit(&mut self) -> Option<CommandResult> {
        let request_id = format!("property-edit-{}", self.frame_index + 1);
        let Ok((_report, command)) = self.authoring_workspace.commit_property_edit(request_id)
        else {
            return None;
        };
        Some(self.dispatch_command(command))
    }

    pub fn dispatch_command(&mut self, command: UiCommand) -> CommandResult {
        if matches!(command.payload, UiCommandPayload::Play)
            && !self.session.has_active_editor_runtime_play_instance()
            && !self.session.has_prepared_editor_play_report()
        {
            return self.begin_editor_play_preparation(command);
        }
        match &command.payload {
            UiCommandPayload::ApproveProjectRuntimeTrust { request_id } => {
                return self.dispatch_project_runtime_trust_decision(
                    command.clone(),
                    request_id,
                    Some(ProjectRuntimeTrustDecisionKind::Trusted),
                );
            }
            UiCommandPayload::DenyProjectRuntimeTrust { request_id } => {
                return self.dispatch_project_runtime_trust_decision(
                    command.clone(),
                    request_id,
                    Some(ProjectRuntimeTrustDecisionKind::Denied),
                );
            }
            UiCommandPayload::CancelProjectRuntimeTrust { request_id } => {
                return self.dispatch_project_runtime_trust_decision(
                    command.clone(),
                    request_id,
                    None,
                );
            }
            UiCommandPayload::ApproveGatewayAccessRequest { request_id } => {
                return self.dispatch_gateway_access_decision(
                    command.clone(),
                    request_id.clone(),
                    ai_tool_gateway::GatewayAccessDecision::Approve,
                );
            }
            UiCommandPayload::RejectGatewayAccessRequest { request_id } => {
                return self.dispatch_gateway_access_decision(
                    command.clone(),
                    request_id.clone(),
                    ai_tool_gateway::GatewayAccessDecision::Reject,
                );
            }
            UiCommandPayload::SetGatewayAccessPage { page_index } => {
                return self.dispatch_gateway_access_page(command.clone(), *page_index);
            }
            _ => {}
        }
        let normalized_command = self.authoring_workspace.normalize_command(command);
        self.last_command_id = Some(normalized_command.command_id.clone());
        let result = self.command_system.dispatch(
            normalized_command.clone(),
            &mut self.session,
            &mut self.transaction_service,
        );
        self.authoring_workspace
            .record_command_result(&normalized_command, &result);
        self.last_command_status = Some(result.status);
        self.last_feedback = Some(command_feedback_from_result(&normalized_command, &result));
        if matches!(result.status, CommandStatus::Committed) {
            if matches!(
                normalized_command.payload,
                UiCommandPayload::OpenProject { .. }
                    | UiCommandPayload::CreateProject { .. }
                    | UiCommandPayload::SelectRecentProject { .. }
                    | UiCommandPayload::RefreshRecentProjects
            ) {
                self.project_manager.save_recent_projects(&self.session);
            }
            self.latest_model = EditorUiModelComposer::compose(&self.session);
            self.apply_inspector_context_lock();
            self.sync_gateway_access_requests();
            self.sync_project_runtime_trust_prompt();
            self.sync_project_open_activity();
            self.latest_model.interaction_feedback = self.last_feedback.clone();
            self.authoring_workspace
                .refresh_from_model(&self.latest_model);
            let renderer_config =
                self.renderer_config(self.config.width as f32, self.config.height as f32);
            self.latest_draw_list = self
                .retained_ui_renderer
                .build_draw_list(&self.latest_model, renderer_config);
            self.redraw_requested = true;
        } else {
            self.latest_model = EditorUiModelComposer::compose(&self.session);
            self.apply_inspector_context_lock();
            self.sync_gateway_access_requests();
            self.sync_project_runtime_trust_prompt();
            self.sync_project_open_activity();
            self.latest_model.interaction_feedback = self.last_feedback.clone();
            self.authoring_workspace
                .refresh_from_model(&self.latest_model);
            let renderer_config =
                self.renderer_config(self.config.width as f32, self.config.height as f32);
            self.latest_draw_list = self
                .retained_ui_renderer
                .build_draw_list(&self.latest_model, renderer_config);
            self.redraw_requested = true;
        }
        result
    }

    fn renderer_config(&mut self, width: f32, height: f32) -> UiRendererConfig {
        self.renderer_config_for_window(
            &editor_ui_renderer::WorkspaceWindowId::main(),
            width,
            height,
        )
    }

    fn renderer_config_for_window(
        &mut self,
        window_id: &editor_ui_renderer::WorkspaceWindowId,
        width: f32,
        height: f32,
    ) -> UiRendererConfig {
        let interaction = self.widget_interaction.snapshot().clone();
        let tree = self.retained_ui_renderer.tree();
        let hit_id = |widget_id: Option<&editor_ui_renderer::WidgetId>| {
            widget_id
                .and_then(|id| tree.and_then(|tree| tree.node(id)))
                .and_then(|node| node.hit_region_id.clone())
        };
        let hovered_hit_id = hit_id(interaction.hovered_widget_id.as_ref())
            .or_else(|| self.focus_input.hovered_hit_id.clone());
        let active_hit_id = hit_id(interaction.active_widget_id.as_ref())
            .or_else(|| self.focus_input.pressed_hit_id.clone());
        let focus_visible_hit_id = interaction
            .focus_visible
            .then(|| hit_id(interaction.focused_widget_id.as_ref()))
            .flatten();
        let focused_hit_id = hit_id(interaction.focused_widget_id.as_ref());
        self.workspace_docking.set_inspector_lock_presentation(
            self.session.inspector_context_anchor().is_some(),
            self.inspector_context_lock.is_some(),
        );
        let workspace_snapshot = self
            .workspace_docking
            .snapshot_window(window_id, editor_workspace_rect(width, height))
            .unwrap_or_else(|| {
                self.workspace_docking
                    .snapshot(editor_workspace_rect(width, height))
            });
        let game_view_target = self.session.last_game_view_runtime_frame().map(|frame| {
            engine_runtime::game_view_presentation::GameViewTargetSpec::new(
                frame.width,
                frame.height,
                frame.presentation_scale_policy,
            )
        });
        UiRendererConfig::new(width, height)
            .with_game_view_target(game_view_target)
            .with_control_interaction(
                hovered_hit_id,
                active_hit_id,
                focused_hit_id,
                focus_visible_hit_id,
            )
            .with_toolbar_overflow_open(self.toolbar_overflow_open)
            .with_workspace_menu_open(self.workspace_menu_open)
            .with_language_menu_open(self.language_menu_open)
            .with_workspace_panel_chrome(
                self.workspace_panel_popup
                    .as_ref()
                    .map(|(stack_id, _)| stack_id.clone()),
            )
            .with_localization(self.localization_snapshot.clone())
            .with_workspace_snapshot(workspace_snapshot)
    }

    fn apply_inspector_context_lock(&mut self) {
        if let Some(anchor) = &self.inspector_context_lock {
            self.latest_model.inspector = self.session.build_inspector_model_for_anchor(anchor);
        }
    }

    pub(crate) fn dispatch_project_launcher_command_or_dispatch(
        &mut self,
        command: UiCommand,
    ) -> Option<CommandResult> {
        if let Some(path) = project_open_path(&command.payload) {
            if !path.is_empty() {
                return self.begin_project_open_preparation(command);
            }
        }
        match &command.payload {
            UiCommandPayload::OpenProject { path } if path.is_empty() => {
                let response = self.project_dialog.pick_folder(ProjectFolderDialogRequest {
                    purpose: ProjectFolderDialogPurpose::OpenProject,
                    title: "Open Project".to_string(),
                    initial_directory: self.project_dialog_initial_directory.clone(),
                });
                self.project_manager.last_dialog_response = Some(response.clone());
                match response {
                    ProjectFolderDialogResponse::Selected { path } => self
                        .dispatch_project_launcher_command_or_dispatch(UiCommand {
                            command_id: command.command_id,
                            source: command.source,
                            request_id: command.request_id,
                            payload: UiCommandPayload::OpenProject { path },
                        }),
                    ProjectFolderDialogResponse::Cancelled => None,
                    ProjectFolderDialogResponse::Unavailable { diagnostic } => {
                        self.record_project_dialog_unavailable(&command, diagnostic);
                        None
                    }
                }
            }
            UiCommandPayload::CreateProject { path, name } if path.is_empty() => {
                let response = self.project_dialog.pick_folder(ProjectFolderDialogRequest {
                    purpose: ProjectFolderDialogPurpose::CreateProject,
                    title: "Create Project".to_string(),
                    initial_directory: self.project_dialog_initial_directory.clone(),
                });
                self.project_manager.last_dialog_response = Some(response.clone());
                match response {
                    ProjectFolderDialogResponse::Selected { path } => {
                        let project_name = if name.trim().is_empty() || name == "NewProject" {
                            Path::new(&path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .filter(|name| !name.trim().is_empty())
                                .unwrap_or("NewProject")
                                .to_string()
                        } else {
                            name.clone()
                        };
                        Some(self.dispatch_command(UiCommand {
                            command_id: "create_project".to_string(),
                            source: UiCommandSource::ProjectLauncher,
                            request_id: command.request_id,
                            payload: UiCommandPayload::CreateProject {
                                path,
                                name: project_name,
                            },
                        }))
                    }
                    ProjectFolderDialogResponse::Cancelled => None,
                    ProjectFolderDialogResponse::Unavailable { diagnostic } => {
                        self.record_project_dialog_unavailable(&command, diagnostic);
                        None
                    }
                }
            }
            UiCommandPayload::OpenRuntimePackage { path } if path.is_empty() => {
                let response = self.project_dialog.pick_folder(ProjectFolderDialogRequest {
                    purpose: ProjectFolderDialogPurpose::OpenRuntimePackage,
                    title: "Open Runtime Package".to_string(),
                    initial_directory: self.project_dialog_initial_directory.clone(),
                });
                self.project_manager.last_dialog_response = Some(response.clone());
                match response {
                    ProjectFolderDialogResponse::Selected { path } => {
                        Some(self.dispatch_command(UiCommand {
                            command_id: "open_runtime_package".to_string(),
                            source: UiCommandSource::Toolbar,
                            request_id: command.request_id,
                            payload: UiCommandPayload::OpenRuntimePackage { path },
                        }))
                    }
                    ProjectFolderDialogResponse::Cancelled => None,
                    ProjectFolderDialogResponse::Unavailable { diagnostic } => {
                        self.record_project_dialog_unavailable(&command, diagnostic);
                        None
                    }
                }
            }
            _ => Some(self.dispatch_command(command)),
        }
    }

    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.latest_surface_width = width as f32;
        self.latest_surface_height = height as f32;
        self.redraw_requested = true;
    }

    pub fn latest_model(&self) -> &EditorUiModel {
        &self.latest_model
    }

    pub fn session(&self) -> &EditorSession {
        &self.session
    }

    pub(crate) fn session_mut(&mut self) -> &mut EditorSession {
        &mut self.session
    }

    pub fn with_project_runtime_trust_environment(
        mut self,
        environment: ProjectRuntimeTrustEnvironment,
    ) -> Self {
        self.project_runtime_trust_environment = Some(environment);
        self
    }

    pub fn install_project_runtime_trust_environment(
        &mut self,
        environment: ProjectRuntimeTrustEnvironment,
    ) {
        self.project_runtime_trust_environment = Some(environment);
    }

    pub fn take_approved_project_runtime_trust_request(
        &mut self,
    ) -> Option<ApprovedProjectRuntimeTrustRequest> {
        self.approved_project_runtime_trust.take()
    }

    pub fn install_project_runtime_preparer(
        &mut self,
        preparer: Arc<dyn ProjectRuntimePreparationAdapter>,
    ) {
        self.project_runtime_preparation_adapter = Some(preparer);
    }

    fn begin_project_open_preparation(&mut self, command: UiCommand) -> Option<CommandResult> {
        if self.project_open_preparation_worker.is_some() {
            return Some(native_host_result(
                &command.command_id,
                &command.request_id,
                CommandStatus::Rejected,
                "editor.project_open.busy",
                "A project is already being prepared. Wait for the active operation to finish.",
            ));
        }
        self.cancel_project_runtime_preparation();
        let Some(path) = project_open_path(&command.payload).filter(|path| !path.is_empty()) else {
            return Some(native_host_result(
                &command.command_id,
                &command.request_id,
                CommandStatus::Rejected,
                "editor.project_open.path_missing",
                "Project preparation requires a non-empty project path.",
            ));
        };
        let project_root = PathBuf::from(path);
        let project_display_name = project_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Project")
            .to_string();
        let operation_id = format!(
            "project-open-{}-{}",
            command.request_id,
            self.frame_index + 1
        );
        let (sender, receiver) = mpsc::channel();
        let worker_root = project_root.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let adapter = self.project_open_preparation_adapter.clone();
        let join = std::thread::Builder::new()
            .name("project-open-preparation".to_string())
            .spawn(move || {
                let progress_sender = sender.clone();
                let result = adapter.prepare(
                    &worker_root,
                    &mut move |phase| {
                        let _ = progress_sender.send(ProjectOpenPreparationEvent::Progress(phase));
                    },
                    &worker_cancelled,
                );
                let _ = sender.send(ProjectOpenPreparationEvent::Completed(result));
            });
        let join = match join {
            Ok(join) => join,
            Err(error) => {
                return Some(native_host_result(
                    &command.command_id,
                    &command.request_id,
                    CommandStatus::Failed,
                    "editor.project_open.worker_start_failed",
                    error.to_string(),
                ));
            }
        };
        self.project_open_preparation_worker = Some(ProjectOpenPreparationWorker {
            receiver,
            join: Some(join),
            command,
            activity: ProjectOpenActivityModel {
                operation_id,
                project_display_name,
                phase: ProjectOpenActivityPhase::ReadingProject,
                completed_units: None,
                total_units: None,
                elapsed_ms: 0,
                cancellable: false,
                diagnostic_code: None,
                next_action: None,
            },
            started_at: Instant::now(),
            cancelled,
        });
        self.sync_project_open_activity();
        self.redraw_requested = true;
        None
    }

    #[cfg(test)]
    pub(crate) fn install_project_open_preparation_adapter(
        &mut self,
        adapter: Arc<dyn ProjectOpenPreparationAdapter>,
    ) {
        self.project_open_preparation_adapter = adapter;
    }

    pub(crate) fn install_editor_play_preparation_adapter(
        &mut self,
        adapter: Arc<dyn EditorPlayPreparationAdapter>,
    ) {
        self.editor_play_preparation_adapter = adapter;
    }

    fn begin_editor_play_preparation(&mut self, command: UiCommand) -> CommandResult {
        if !self.active_project_uses_linked_editor_composition() {
            if let Some(blocker) = self.session.project_runtime_play_blocker() {
                return native_host_result(
                    &command.command_id,
                    &command.request_id,
                    CommandStatus::Rejected,
                    &blocker.code,
                    blocker.message,
                );
            }
        }
        if self.editor_play_preparation_worker.is_some() {
            return native_host_result(
                &command.command_id,
                &command.request_id,
                CommandStatus::Rejected,
                "editor.play_preparation.busy",
                "Editor Play is already being prepared. Wait for the active operation to finish.",
            );
        }
        let ticket = match self.session.editor_play_preparation_ticket() {
            Ok(Some(ticket)) => ticket,
            Ok(None) => {
                return self.command_system.dispatch(
                    command,
                    &mut self.session,
                    &mut self.transaction_service,
                );
            }
            Err(error) => {
                return native_host_result(
                    &command.command_id,
                    &command.request_id,
                    CommandStatus::Failed,
                    &error.code,
                    error.message,
                );
            }
        };
        let (sender, receiver) = mpsc::channel();
        let worker_ticket = ticket.clone();
        let adapter = self.editor_play_preparation_adapter.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let join = std::thread::Builder::new()
            .name("editor-play-preparation".to_string())
            .spawn(move || {
                let report = adapter.prepare(&worker_ticket, &worker_cancelled);
                let _ = sender.send(report);
            });
        let join = match join {
            Ok(join) => join,
            Err(error) => {
                return native_host_result(
                    &command.command_id,
                    &command.request_id,
                    CommandStatus::Failed,
                    "editor.play_preparation.worker_start_failed",
                    error.to_string(),
                );
            }
        };
        self.editor_play_preparation_worker = Some(EditorPlayPreparationWorker {
            receiver,
            join: Some(join),
            command: command.clone(),
            ticket,
            started_at: Instant::now(),
            cancelled,
        });
        let result = native_host_result(
            &command.command_id,
            &command.request_id,
            CommandStatus::Pending,
            "editor.play_preparation.started",
            "Editor Play preparation is running in the background.",
        );
        self.last_command_id = Some(command.command_id.clone());
        self.last_command_status = Some(result.status);
        self.last_feedback = Some(command_feedback_from_result(&command, &result));
        self.sync_editor_play_activity();
        self.redraw_requested = true;
        result
    }

    fn sync_editor_play_activity(&mut self) {
        let Some(worker) = self.editor_play_preparation_worker.as_ref() else {
            return;
        };
        if let Some(play) = self
            .latest_model
            .toolbar
            .commands
            .iter_mut()
            .find(|command| command.command_id == "play")
        {
            play.enabled = false;
            play.reason_disabled = Some("editor.play_preparation.busy".to_string());
        }
        let elapsed_seconds = worker.started_at.elapsed().as_secs();
        let message = if self.localization_snapshot.locale.as_str() == "zh-CN" {
            format!("正在准备运行：构建预览运行包（已用时 {elapsed_seconds} 秒）")
        } else {
            format!("Preparing Play: building preview package ({elapsed_seconds}s elapsed)")
        };
        self.last_feedback = Some(EditorCommandFeedback {
            command_id: worker.command.command_id.clone(),
            status: editor_ui_model::EditorCommandFeedbackStatus::Info,
            diagnostic_code: Some("editor.play_preparation.running".to_string()),
            message,
            reason: None,
            source: worker.command.source.clone(),
        });
    }

    fn pump_editor_play_preparation(&mut self) -> bool {
        let event = match self.editor_play_preparation_worker.as_ref() {
            Some(worker) => match worker.receiver.try_recv() {
                Ok(report) => Some(report.map_err(|error| (error.code, error.message))),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err((
                    "editor.play_preparation.worker_disconnected".to_string(),
                    "Editor Play preparation worker disconnected without a report.".to_string(),
                ))),
            },
            None => None,
        };
        let Some(event) = event else {
            return false;
        };
        let mut worker = self
            .editor_play_preparation_worker
            .take()
            .expect("play preparation worker exists");
        if let Some(join) = worker.join.take() {
            let _ = join.join();
        }
        let result = match event {
            Ok(report) if !worker.cancelled.load(Ordering::Acquire) => self
                .session
                .install_prepared_editor_play_report(&worker.ticket, report)
                .map_err(|error| (error.code, error.message)),
            Ok(_) => Err((
                "editor.play_preparation.cancelled".to_string(),
                "Editor Play preparation was cancelled.".to_string(),
            )),
            Err((code, message)) => Err((code, message)),
        };
        match result {
            Ok(()) => {
                let _elapsed_ms = worker.started_at.elapsed().as_millis() as u64;
                self.dispatch_command(worker.command);
            }
            Err((code, message)) => {
                let result = native_host_result(
                    &worker.command.command_id,
                    &worker.command.request_id,
                    CommandStatus::Failed,
                    &code,
                    message,
                );
                self.last_command_status = Some(result.status);
                self.last_feedback = Some(command_feedback_from_result(&worker.command, &result));
            }
        }
        true
    }

    fn pump_project_open_preparation(&mut self) -> bool {
        let event = {
            let Some(worker) = self.project_open_preparation_worker.as_mut() else {
                return false;
            };
            worker.activity.elapsed_ms = worker.started_at.elapsed().as_millis() as u64;
            match worker.receiver.try_recv() {
                Ok(event) => Some(event),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => Some(ProjectOpenPreparationEvent::Completed(
                    Err(ProjectOpenPreparationError {
                        code: "editor.project_open.worker_disconnected".to_string(),
                        message: "Project preparation worker disconnected before completion."
                            .to_string(),
                        path: None,
                        next_action: "Retry opening the project.".to_string(),
                    }),
                )),
            }
        };
        match event.expect("project open event exists") {
            ProjectOpenPreparationEvent::Progress(phase) => {
                if let Some(worker) = self.project_open_preparation_worker.as_mut() {
                    worker.activity.phase = match phase {
                        ProjectOpenPreparationPhase::ReadingProject => {
                            ProjectOpenActivityPhase::ReadingProject
                        }
                        ProjectOpenPreparationPhase::ComputingDigest => {
                            ProjectOpenActivityPhase::ComputingDigest
                        }
                    };
                }
                true
            }
            ProjectOpenPreparationEvent::Completed(result) => {
                let mut worker = self
                    .project_open_preparation_worker
                    .take()
                    .expect("completed project open worker exists");
                if let Some(join) = worker.join.take() {
                    let _ = join.join();
                }
                match result {
                    Ok(prepared) => {
                        let project_root = prepared.project_root.clone();
                        self.session.install_prepared_project_open(prepared);
                        let command = worker.command;
                        let result = self.dispatch_command(command);
                        self.session.clear_prepared_project_open();
                        if result.status == CommandStatus::Committed {
                            self.begin_project_runtime_after_authoring_open(&project_root);
                        }
                    }
                    Err(error) => {
                        let result = native_host_result(
                            &worker.command.command_id,
                            &worker.command.request_id,
                            CommandStatus::Failed,
                            error.code,
                            error.message,
                        );
                        self.last_command_id = Some(worker.command.command_id.clone());
                        self.last_command_status = Some(result.status);
                        self.last_feedback =
                            Some(command_feedback_from_result(&worker.command, &result));
                    }
                }
                true
            }
        }
    }

    fn sync_project_open_activity(&mut self) {
        let activity = self.project_open_preparation_worker.as_ref().map(|worker| {
            let mut activity = worker.activity.clone();
            activity.elapsed_ms = worker.started_at.elapsed().as_millis() as u64;
            activity
        });
        self.latest_model.project_launcher.activity = activity;
        if self.latest_model.project_launcher.activity.is_some() {
            for command in &mut self.latest_model.project_launcher.commands {
                command.enabled = false;
                command.reason_disabled = Some("editor.project_open.busy".to_string());
            }
        }
    }

    fn begin_project_runtime_after_authoring_open(&mut self, project_root: &Path) {
        let Some(project) = self.session.active_project_session() else {
            return;
        };
        let requested = &project.manifest.runtime_module;
        if requested.resolved_source_kind() != ProjectRuntimeSourceKind::ProjectRust {
            return;
        }
        if self.active_project_uses_linked_editor_composition() {
            return;
        }
        self.session.await_project_runtime_trust(
            project.manifest.project_id.clone(),
            requested.module_id.clone(),
            requested.interface_version.clone(),
        );
        match self.review_project_runtime_trust(
            project_root,
            "project_runtime_preparation",
            &format!("project-runtime-open-{}", self.frame_index + 1),
        ) {
            ProjectRuntimeTrustReview::Continue => {
                self.begin_approved_project_runtime_preparation();
            }
            ProjectRuntimeTrustReview::Prompted => {}
            ProjectRuntimeTrustReview::Rejected(result) => {
                let diagnostic = result.diagnostics.first();
                let failure = ProjectRuntimeNativeModuleDiagnostic {
                    code: diagnostic
                        .map(|value| value.code.clone())
                        .unwrap_or_else(|| "project_runtime.trust_rejected".to_string()),
                    stage: "trust".to_string(),
                    message: diagnostic
                        .map(|value| value.message.clone())
                        .unwrap_or_else(|| {
                            "Project Runtime trust review was rejected.".to_string()
                        }),
                    path: Some(project_root.display().to_string()),
                    next_action: "Review the ProjectRust identity and approve it before Play."
                        .to_string(),
                };
                if let Some(ticket) = match self.session.project_runtime_preparation_state() {
                    editor_core::ProjectRuntimePreparationState::AwaitingTrust(ticket) => {
                        Some(ticket.clone())
                    }
                    _ => None,
                } {
                    self.session
                        .fail_project_runtime_preparation(&ticket, failure);
                }
            }
        }
    }

    fn active_project_uses_linked_editor_composition(&self) -> bool {
        let Some(project) = self.session.active_project_session() else {
            return false;
        };
        let Some(identity) = self.session.project_editor_composition_identity() else {
            return false;
        };
        let requested = &project.manifest.runtime_module;
        requested.resolved_source_kind() == ProjectRuntimeSourceKind::ProjectRust
            && identity.project_id == project.manifest.project_id
            && identity.module_id == requested.module_id
            && identity.interface_version == requested.interface_version
    }

    fn sync_project_editor_composition_actionability(&mut self) {
        if !self.active_project_uses_linked_editor_composition()
            || self.latest_model.toolbar.runtime_state == editor_ui_model::RuntimeRunState::Playing
        {
            return;
        }
        let Some(play) = self
            .latest_model
            .toolbar
            .commands
            .iter_mut()
            .find(|command| command.command_id == "play")
        else {
            return;
        };
        if play
            .reason_disabled
            .as_deref()
            .is_some_and(|reason| reason.starts_with("project_runtime."))
        {
            play.enabled = true;
            play.reason_disabled = None;
        }
    }

    fn begin_approved_project_runtime_preparation(&mut self) {
        let Some(approved) = self.approved_project_runtime_trust.take() else {
            return;
        };
        let Some(project) = self.session.active_project_session() else {
            return;
        };
        let active_root = project
            .project_root
            .canonicalize()
            .unwrap_or_else(|_| project.project_root.clone());
        if active_root != approved.project_root
            || project.manifest.project_id != approved.trust_request.project_id
        {
            return;
        }
        let requested = &project.manifest.runtime_module;
        let ticket = self.session.begin_project_runtime_preparation(
            project.manifest.project_id.clone(),
            requested.module_id.clone(),
            requested.interface_version.clone(),
        );
        let Some(preparer) = self.project_runtime_preparation_adapter.clone() else {
            self.session.fail_project_runtime_preparation(
                &ticket,
                ProjectRuntimeNativeModuleDiagnostic {
                    code: "project_runtime.preparer_unavailable".to_string(),
                    stage: "prepare".to_string(),
                    message: "The stable Editor has no native project runtime preparer."
                        .to_string(),
                    path: Some(approved.project_root.display().to_string()),
                    next_action: "Install the production ProjectRuntime preparation service."
                        .to_string(),
                },
            );
            return;
        };
        self.cancel_project_runtime_worker_only();
        let (sender, receiver) = mpsc::channel();
        let control = ProjectRuntimeNativeModuleBuildControl::default();
        let worker_control = control.clone();
        let join = std::thread::Builder::new()
            .name("project-runtime-prepare".to_string())
            .spawn(move || {
                let progress_sender = sender.clone();
                let result = preparer.prepare(approved, worker_control, &mut move |phase| {
                    let _ = progress_sender.send(ProjectRuntimePreparationEvent::Progress(phase));
                });
                let _ = sender.send(ProjectRuntimePreparationEvent::Completed(result));
            });
        match join {
            Ok(join) => {
                self.project_runtime_preparation_worker = Some(ProjectRuntimePreparationWorker {
                    receiver,
                    join: Some(join),
                    ticket,
                    control,
                });
            }
            Err(error) => {
                self.session.fail_project_runtime_preparation(
                    &ticket,
                    ProjectRuntimeNativeModuleDiagnostic {
                        code: "project_runtime.worker_start_failed".to_string(),
                        stage: "prepare".to_string(),
                        message: error.to_string(),
                        path: None,
                        next_action: "Retry opening the project.".to_string(),
                    },
                );
            }
        }
        self.redraw_requested = true;
    }

    fn pump_project_runtime_preparation(&mut self) -> bool {
        let event = {
            let Some(worker) = self.project_runtime_preparation_worker.as_ref() else {
                return false;
            };
            match worker.receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => ProjectRuntimePreparationEvent::Completed(Err(
                    ProjectRuntimeNativeModuleDiagnostic {
                        code: "project_runtime.worker_disconnected".to_string(),
                        stage: "prepare".to_string(),
                        message: "Native project runtime worker disconnected before completion."
                            .to_string(),
                        path: None,
                        next_action: "Retry opening the project.".to_string(),
                    },
                )),
            }
        };
        match event {
            ProjectRuntimePreparationEvent::Progress(_phase) => true,
            ProjectRuntimePreparationEvent::Completed(result) => {
                let mut worker = self
                    .project_runtime_preparation_worker
                    .take()
                    .expect("completed project runtime worker exists");
                if let Some(join) = worker.join.take() {
                    let _ = join.join();
                }
                match result {
                    Ok(prepared) => {
                        self.session.install_prepared_project_runtime(
                            &worker.ticket,
                            prepared.identity,
                            prepared.linked_project_runtimes,
                        );
                    }
                    Err(diagnostic) => {
                        self.session
                            .fail_project_runtime_preparation(&worker.ticket, diagnostic);
                    }
                }
                self.redraw_requested = true;
                true
            }
        }
    }

    fn cancel_project_runtime_worker_only(&mut self) {
        if let Some(mut worker) = self.project_runtime_preparation_worker.take() {
            worker.control.request_cancel();
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
    }

    pub fn cancel_project_runtime_preparation(&mut self) {
        self.cancel_project_runtime_worker_only();
        self.approved_project_runtime_trust = None;
        self.pending_project_runtime_trust = None;
        self.session.cancel_project_runtime_preparation();
        self.redraw_requested = true;
    }

    fn review_project_runtime_trust(
        &mut self,
        path: &Path,
        command_id: &str,
        request_id: &str,
    ) -> ProjectRuntimeTrustReview {
        let Some(environment) = self.project_runtime_trust_environment.as_ref() else {
            return ProjectRuntimeTrustReview::Continue;
        };
        let manifest_path = path.join("project.aife.json");
        let Ok(manifest_bytes) = std::fs::read(&manifest_path) else {
            return ProjectRuntimeTrustReview::Continue;
        };
        let Ok(manifest) = serde_json::from_slice::<ProjectManifest>(&manifest_bytes) else {
            return ProjectRuntimeTrustReview::Continue;
        };
        if manifest.runtime_module.resolved_source_kind() != ProjectRuntimeSourceKind::ProjectRust {
            return ProjectRuntimeTrustReview::Continue;
        }
        let inspection = match ProjectRuntimeTrustInspection::inspect(
            path,
            &environment.engine_sdk_root,
            environment.editor_build_identity.clone(),
        ) {
            Ok(value) => value,
            Err(error) => {
                return ProjectRuntimeTrustReview::Rejected(native_host_result(
                    command_id,
                    request_id,
                    CommandStatus::Failed,
                    error.code,
                    error.message,
                ));
            }
        };
        let evaluation = match environment.trust_module.evaluate(&inspection.request, None) {
            Ok(value) => value,
            Err(error) => {
                return ProjectRuntimeTrustReview::Rejected(native_host_result(
                    command_id,
                    request_id,
                    CommandStatus::Failed,
                    error.code,
                    error.message,
                ));
            }
        };
        if evaluation.status == ProjectRuntimeTrustStatus::Trusted {
            self.approved_project_runtime_trust = Some(ApprovedProjectRuntimeTrustRequest {
                project_root: inspection.canonical_project_root,
                trust_request: inspection.request,
            });
            self.redraw_requested = true;
            return ProjectRuntimeTrustReview::Continue;
        }
        if evaluation.status == ProjectRuntimeTrustStatus::Denied {
            return ProjectRuntimeTrustReview::Rejected(native_host_result(
                command_id,
                request_id,
                CommandStatus::Rejected,
                "project_editor_composition.trust_denied",
                "Project Runtime execution remains denied for this exact identity.",
            ));
        }
        self.pending_project_runtime_trust = Some(PendingProjectRuntimeTrust {
            inspection,
            identity_changed: evaluation.status == ProjectRuntimeTrustStatus::Stale,
        });
        self.sync_project_runtime_trust_prompt();
        self.redraw_requested = true;
        ProjectRuntimeTrustReview::Prompted
    }

    fn dispatch_project_runtime_trust_decision(
        &mut self,
        command: UiCommand,
        request_id: &str,
        decision: Option<ProjectRuntimeTrustDecisionKind>,
    ) -> CommandResult {
        let expected_request_id = self
            .pending_project_runtime_trust
            .as_ref()
            .map(|pending| project_runtime_trust_request_id(&pending.inspection.request));
        if expected_request_id.as_deref() != Some(request_id) {
            return native_host_result(
                &command.command_id,
                &command.request_id,
                CommandStatus::Rejected,
                "project_editor_composition.trust_prompt_stale",
                "Project Runtime trust prompt is no longer active or its identity changed.",
            );
        }
        let Some(pending) = self.pending_project_runtime_trust.take() else {
            unreachable!("validated pending trust prompt exists");
        };
        let result = if let Some(decision) = decision {
            match self.project_runtime_trust_environment.as_ref() {
                Some(environment) => match environment.trust_module.record_explicit(
                    &pending.inspection.request,
                    decision,
                    epoch_ms(),
                ) {
                    Ok(_) => {
                        if decision == ProjectRuntimeTrustDecisionKind::Trusted {
                            self.approved_project_runtime_trust =
                                Some(ApprovedProjectRuntimeTrustRequest {
                                    project_root: pending.inspection.canonical_project_root,
                                    trust_request: pending.inspection.request,
                                });
                        }
                        native_host_result(
                            &command.command_id,
                            &command.request_id,
                            CommandStatus::Committed,
                            "project_editor_composition.trust_decision_recorded",
                            if decision == ProjectRuntimeTrustDecisionKind::Trusted {
                                "Project Runtime identity approved; composition preparation may begin."
                            } else {
                                "Project Runtime identity denied; no build or launch was started."
                            },
                        )
                    }
                    Err(error) => native_host_result(
                        &command.command_id,
                        &command.request_id,
                        CommandStatus::Failed,
                        error.code,
                        error.message,
                    ),
                },
                None => native_host_result(
                    &command.command_id,
                    &command.request_id,
                    CommandStatus::Failed,
                    "project_editor_composition.trust_store_unavailable",
                    "Project Runtime trust environment is unavailable.",
                ),
            }
        } else {
            native_host_result(
                &command.command_id,
                &command.request_id,
                CommandStatus::Committed,
                "project_editor_composition.trust_cancelled",
                "Project Runtime trust review was cancelled; no receipt, build, or launch was created.",
            )
        };
        if result.status == CommandStatus::Committed
            && decision == Some(ProjectRuntimeTrustDecisionKind::Trusted)
        {
            self.begin_approved_project_runtime_preparation();
        } else if result.status == CommandStatus::Committed {
            if let editor_core::ProjectRuntimePreparationState::AwaitingTrust(ticket) =
                self.session.project_runtime_preparation_state()
            {
                let ticket = ticket.clone();
                self.session.fail_project_runtime_preparation(
                    &ticket,
                    ProjectRuntimeNativeModuleDiagnostic {
                        code: if decision == Some(ProjectRuntimeTrustDecisionKind::Denied) {
                            "project_runtime.trust_denied".to_string()
                        } else {
                            "project_runtime.trust_cancelled".to_string()
                        },
                        stage: "trust".to_string(),
                        message:
                            "Project Runtime preparation cannot continue without trust approval."
                                .to_string(),
                        path: None,
                        next_action:
                            "Reopen the project and approve its current ProjectRust identity."
                                .to_string(),
                    },
                );
            }
        }
        self.last_command_id = Some(command.command_id.clone());
        self.last_command_status = Some(result.status);
        self.last_feedback = Some(command_feedback_from_result(&command, &result));
        self.latest_model = EditorUiModelComposer::compose(&self.session);
        self.sync_gateway_access_requests();
        self.sync_project_runtime_trust_prompt();
        self.latest_model.interaction_feedback = self.last_feedback.clone();
        self.redraw_requested = true;
        result
    }

    fn sync_project_runtime_trust_prompt(&mut self) {
        self.latest_model.project_runtime_trust_prompt = self
            .pending_project_runtime_trust
            .as_ref()
            .map(|pending| editor_ui_model::ProjectRuntimeTrustPromptModel {
                request_id: project_runtime_trust_request_id(&pending.inspection.request),
                project_name: pending.inspection.project_name.clone(),
                canonical_project_root: pending
                    .inspection
                    .canonical_project_root
                    .display()
                    .to_string(),
                module_id: pending.inspection.module_id.clone(),
                dependency_summary: pending.inspection.dependency_summary.clone(),
                identity_changed: pending.identity_changed,
            });
    }

    pub fn gateway_client(&self) -> ai_tool_gateway::GatewayOwnerThreadClient {
        self.gateway_client.clone()
    }

    pub fn request_gateway_goal_mutation_access(
        &mut self,
        client_session_id: &str,
        goal_binding: editor_core::AiGoalBinding,
        risk_envelope: editor_core::AiRiskEnvelope,
    ) -> Result<ai_tool_gateway::GatewayAccessRequest, ai_tool_gateway::GatewayControlError> {
        let request = self.gateway_core.request_goal_mutation_access(
            &self.session,
            client_session_id,
            goal_binding,
            risk_envelope,
        )?;
        self.sync_gateway_access_requests();
        self.redraw_requested = true;
        Ok(request)
    }

    pub fn gateway_host_binding(&self) -> Option<&ai_tool_gateway::EditorGatewayHostBinding> {
        self.gateway_host.as_ref().map(|host| host.binding())
    }

    pub fn editor_instance_id(&self) -> &str {
        &self.editor_instance_id
    }

    pub fn gateway_discovery_path(&self) -> Option<&Path> {
        self.gateway_host.as_ref().map(|host| host.discovery_path())
    }

    pub fn gateway_host_error(&self) -> Option<&ai_tool_gateway::GatewayControlError> {
        self.gateway_host_error.as_ref()
    }

    pub fn last_gateway_access_decision_receipt(
        &self,
    ) -> Option<&ai_tool_gateway::GatewayAccessDecisionReceipt> {
        self.last_gateway_access_decision_receipt.as_ref()
    }

    pub fn last_gateway_grant_receipt(
        &self,
    ) -> Option<&ai_tool_gateway::GatewayAccessDecisionReceipt> {
        self.last_gateway_access_decision_receipt()
    }

    pub fn last_gateway_requests_processed(&self) -> usize {
        self.last_gateway_requests_processed
    }

    fn reconcile_gateway_host(&mut self) {
        if !self.gateway_host_enabled {
            return;
        }
        let desired_binding = ai_tool_gateway::EditorGatewayHostBinding {
            editor_instance_id: self.editor_instance_id.clone(),
        };
        if self
            .gateway_host
            .as_ref()
            .map(|host| host.binding() == &desired_binding)
            .unwrap_or(false)
        {
            return;
        }
        self.gateway_host.take();
        if self.gateway_host_attempted_binding.as_ref() == Some(&desired_binding) {
            return;
        }
        self.gateway_host_attempted_binding = Some(desired_binding.clone());
        let started = match &self.gateway_discovery_root_override {
            Some(root) => ai_tool_gateway::EditorGatewayHost::start_in_root(
                root,
                desired_binding.editor_instance_id.clone(),
                self.gateway_client.clone(),
            ),
            None => ai_tool_gateway::EditorGatewayHost::start(
                desired_binding.editor_instance_id.clone(),
                self.gateway_client.clone(),
            ),
        };
        match started {
            Ok(host) => {
                self.gateway_host_error = None;
                self.gateway_host = Some(host);
            }
            Err(error) => {
                self.gateway_host_error = Some(error);
            }
        }
    }

    fn sync_gateway_access_requests(&mut self) {
        const PAGE_SIZE: usize = 2;

        let now = epoch_ms();
        let _ = self.gateway_core.prune(&self.session, now);
        let requests = self.gateway_core.approval_inbox(now);
        let total_count = requests.len();
        let page_count = total_count.div_ceil(PAGE_SIZE);
        self.gateway_access_page = self.gateway_access_page.min(page_count.saturating_sub(1));
        let page_start = self.gateway_access_page.saturating_mul(PAGE_SIZE);
        let page_end = (page_start + PAGE_SIZE).min(total_count);
        let page_requests = requests
            .get(page_start..page_end)
            .unwrap_or_default()
            .iter()
            .map(|request| GatewayAccessRequestModel {
                request_id: request.request_id.clone(),
                operation_short_id: request
                    .operation_id
                    .as_deref()
                    .map(short_gateway_session_id)
                    .unwrap_or_else(|| "manual".to_string()),
                client_session_id: request.client_session_id.clone(),
                session_short_id: short_gateway_session_id(&request.client_session_id),
                client_kind: gateway_client_kind_label(request.client_kind).to_string(),
                client_version: request.client_version.clone(),
                project_identity: request.project_identity.clone(),
                connected_age_ms: now.saturating_sub(request.connected_at_epoch_ms),
                expires_in_ms: request.expires_at_epoch_ms.saturating_sub(now),
                state: "awaiting_user".to_string(),
                requested_profile: request.requested_profile.clone(),
                risk_class: format!("{:?}", request.risk_envelope.risk_class),
                capabilities: request.capabilities.clone(),
                blocked_capabilities: request.blocked_capabilities.clone(),
                goal_id: request.goal_binding.goal_id.clone(),
                user_visible_outcome: request.goal_binding.user_visible_outcome.clone(),
                completion_policy: format!("{:?}", request.goal_binding.completion_policy),
                allowed_paths: request.risk_envelope.allowed_paths.clone(),
                denied_paths: request.risk_envelope.denied_paths.clone(),
                allowed_objects: request.risk_envelope.allowed_objects.clone(),
                max_mutation_count: request.risk_envelope.max_mutation_count,
                time_budget_ms: request.risk_envelope.time_budget_ms,
                external_cost_budget_microunits: request
                    .risk_envelope
                    .external_cost_budget_microunits,
                allow_delete: request.risk_envelope.allow_delete,
                allow_dependency_change: request.risk_envelope.allow_dependency_change,
                allow_network: request.risk_envelope.allow_network,
                approval_digest: request.approval_digest.clone(),
            })
            .collect();
        self.latest_model.ai_panel.gateway_access = editor_ui_model::GatewayAccessInboxModel {
            requests: page_requests,
            page_index: self.gateway_access_page,
            page_count,
            total_count,
        };
    }

    fn dispatch_gateway_access_decision(
        &mut self,
        command: UiCommand,
        request_id: String,
        decision: ai_tool_gateway::GatewayAccessDecision,
    ) -> CommandResult {
        let command_id = if decision == ai_tool_gateway::GatewayAccessDecision::Approve {
            "approve_gateway_access_request"
        } else {
            "reject_gateway_access_request"
        };
        let status = match self.gateway_core.decide_access(
            &self.session,
            &request_id,
            decision,
            "native-editor-user",
            epoch_ms(),
        ) {
            Ok(receipt) => {
                self.last_gateway_access_decision_receipt = Some(receipt);
                CommandStatus::Committed
            }
            Err(error) => {
                self.gateway_host_error = Some(error);
                CommandStatus::Rejected
            }
        };
        self.last_command_id = Some(command_id.to_string());
        self.last_command_status = Some(status);
        self.latest_model = EditorUiModelComposer::compose(&self.session);
        self.apply_inspector_context_lock();
        self.sync_gateway_access_requests();
        self.redraw_requested = true;
        CommandResult {
            transaction_id: format!("gateway-access-decision-{}", self.frame_index),
            request_id: command.request_id,
            command_id: command_id.to_string(),
            status,
            diagnostics: Vec::new(),
            console_entries: Vec::new(),
            state_changes: Vec::new(),
            ui_model_revision: self.latest_model.revision,
        }
    }

    fn dispatch_gateway_access_page(
        &mut self,
        command: UiCommand,
        page_index: usize,
    ) -> CommandResult {
        self.gateway_access_page = page_index;
        self.sync_gateway_access_requests();
        self.last_command_id = Some("set_gateway_access_page".to_string());
        self.last_command_status = Some(CommandStatus::Committed);
        self.redraw_requested = true;
        CommandResult {
            transaction_id: format!("gateway-access-page-{}", self.frame_index),
            request_id: command.request_id,
            command_id: "set_gateway_access_page".to_string(),
            status: CommandStatus::Committed,
            diagnostics: Vec::new(),
            console_entries: Vec::new(),
            state_changes: Vec::new(),
            ui_model_revision: self.latest_model.revision,
        }
    }

    pub fn tick_active_game_view_runtime_descriptor_frame(
        &mut self,
    ) -> Option<GameViewPresentReport> {
        self.session
            .tick_active_game_view_runtime_descriptor_frame()
    }

    pub fn tick_active_game_view_runtime_descriptor_frame_with_fixed_steps(
        &mut self,
        fixed_step_count: usize,
    ) -> Option<GameViewPresentReport> {
        self.session
            .tick_active_game_view_runtime_descriptor_frame_with_fixed_steps(fixed_step_count)
    }

    pub fn active_game_view_frame_for_window_present(&self) -> Option<GameViewRuntimeFrame> {
        self.session.last_game_view_runtime_frame().cloned()
    }

    pub fn pending_project_preview_frame_ticket(&self) -> Option<&ProjectPreviewFrameTicket> {
        self.session.pending_project_preview_frame_ticket()
    }

    pub fn record_project_preview_presented_frame(
        &mut self,
        readback: ProjectPreviewFrameReadback,
    ) -> Result<ProjectPreviewFrameEvidence, ProjectPreviewEvidenceError> {
        self.session
            .record_project_preview_presented_frame(readback)
    }

    pub fn fail_project_preview_frame_capture(
        &mut self,
        operation_id: &str,
        diagnostic_code: impl Into<String>,
        diagnostic_message: impl Into<String>,
    ) -> bool {
        self.session.fail_project_preview_frame_capture(
            operation_id,
            diagnostic_code,
            diagnostic_message,
        )
    }

    pub fn active_game_view_rhi_command_plan(
        &self,
    ) -> Option<&engine_runtime::rhi_command_plan::RhiCommandPlan> {
        self.session.active_game_view_rhi_command_plan()
    }

    pub fn active_game_view_font_bundles(
        &self,
    ) -> Option<&engine_runtime::font_bundle::RuntimeFontBundleRegistry> {
        self.session.active_game_view_font_bundles()
    }

    pub fn active_game_view_runtime_texture_uploads(
        &self,
    ) -> Option<&engine_runtime::runtime_texture::RuntimeTextureUploadRegistry> {
        self.session.active_game_view_runtime_texture_uploads()
    }

    pub(crate) fn game_view_aui_action_logical_point(
        &self,
        node_id: &str,
    ) -> Result<(UiPoint, String, crate::GameViewAuiCoordinateEvidence), String> {
        self.viewport_host
            .game_viewport()
            .ok_or_else(|| "authority.game_viewport_missing".to_string())?;
        let matches = self
            .session
            .active_game_view_aui_action_targets()
            .iter()
            .filter(|target| target.node_id == node_id)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "authority.aui_target_not_unique:{node_id}:{}",
                matches.len()
            ));
        }
        let target = matches[0];
        let rect = target
            .actionable_rect()
            .ok_or_else(|| format!("authority.aui_target_not_actionable:{node_id}"))?;
        if target.reference_width == 0 || target.reference_height == 0 {
            return Err(format!("authority.aui_target_extent_invalid:{node_id}"));
        }
        let presentation = self
            .viewport_host
            .game_presentation()
            .ok_or_else(|| "authority.game_view_presentation_missing".to_string())?;
        let reference_point = engine_runtime::game_view_presentation::GameViewPoint::new(
            rect.x + rect.width * 0.5,
            rect.y + rect.height * 0.5,
        );
        let target_point = presentation
            .reference_to_target(target.canvas_id.as_str(), reference_point)
            .map_err(|error| format!("authority.{}", error.code))?;
        let point = presentation
            .target_to_display(target_point)
            .map_err(|error| format!("authority.{}", error.code))?;
        let reference_extent = presentation
            .canvas_reference_extent(target.canvas_id.as_str())
            .ok_or_else(|| format!("authority.aui_canvas_missing:{}", target.canvas_id))?;
        Ok((
            UiPoint {
                x: point.x,
                y: point.y,
            },
            target.action_id.clone(),
            crate::GameViewAuiCoordinateEvidence {
                presentation_identity: presentation.identity.clone(),
                target_extent: presentation.target_extent,
                display_content_rect: presentation.display_content_rect,
                scale_policy: presentation.scale_policy,
                canvas_id: target.canvas_id.clone(),
                reference_extent,
                reference_point,
                target_point,
                display_point: point,
            },
        ))
    }

    pub fn mark_active_game_view_gpu_present_result(
        &mut self,
        gpu_present_status: impl Into<String>,
        shared_gpu_context_status: impl Into<String>,
        diagnostics: Vec<GameViewPresentDiagnostic>,
    ) -> Option<GameViewPresentReport> {
        self.session.mark_active_game_view_gpu_present_result(
            gpu_present_status,
            shared_gpu_context_status,
            diagnostics,
        )
    }

    pub fn latest_draw_list(&self) -> &UiDrawList {
        &self.latest_draw_list
    }

    pub fn retained_ui_renderer(&self) -> &RetainedEditorUiRenderer {
        &self.retained_ui_renderer
    }

    pub fn visible_asset_thumbnail_ids(&self) -> BTreeSet<String> {
        image_texture_ids(&self.latest_draw_list)
    }

    pub fn asset_thumbnail_payloads_for_ids(
        &mut self,
        thumbnail_ids: &BTreeSet<String>,
    ) -> Vec<editor_core::AssetThumbnailCpuPayload> {
        self.session.asset_thumbnail_payloads_for_ids(thumbnail_ids)
    }

    pub fn asset_thumbnail_summary(&self) -> editor_core::AssetThumbnailServiceSummary {
        self.session.asset_thumbnail_summary()
    }

    pub fn workspace_docking(&self) -> &EditorWorkspaceDockingModule {
        &self.workspace_docking
    }

    pub fn workspace_pointer_cursor(&self) -> WorkspacePointerCursor {
        let active_node_id = self
            .workspace_docking
            .active_resize_node_id()
            .map(LayoutNodeId::as_str);
        let hovered_node_id = self
            .focus_input
            .hovered_hit_id
            .as_deref()
            .and_then(|hovered_hit_id| {
                self.latest_draw_list
                    .hit_regions
                    .iter()
                    .find(|region| region.id == hovered_hit_id)
            })
            .and_then(|region| match &region.target {
                HitTarget::WorkspaceSplitter { node_id } => Some(node_id.as_str()),
                _ => None,
            });
        let Some(node_id) = active_node_id.or(hovered_node_id) else {
            return WorkspacePointerCursor::Default;
        };
        let snapshot = self.workspace_docking.snapshot(editor_workspace_rect(
            self.latest_surface_width,
            self.latest_surface_height,
        ));
        match snapshot
            .splitters
            .iter()
            .find(|splitter| splitter.node_id.as_str() == node_id)
            .map(|splitter| splitter.axis)
        {
            Some(DockSplitAxis::Horizontal) => WorkspacePointerCursor::ColumnResize,
            Some(DockSplitAxis::Vertical) => WorkspacePointerCursor::RowResize,
            None => WorkspacePointerCursor::Default,
        }
    }

    pub fn with_workspace_layout_store(mut self, store: WorkspaceLayoutStore) -> Self {
        self.install_workspace_layout_store(store);
        self
    }

    pub fn with_editor_preference_store(mut self, store: EditorPreferenceStore) -> Self {
        self.install_editor_preference_store(store);
        self
    }

    pub fn localization_snapshot(&self) -> &EditorLocalizationSnapshot {
        &self.localization_snapshot
    }

    pub fn localization_diagnostic(&self) -> Option<&EditorCatalogDiagnostic> {
        self.localization_diagnostic.as_ref()
    }

    pub fn change_editor_locale(&mut self, locale: EditorLocaleId) -> EditorLocaleChangeResult {
        let next_revision = self.localization_snapshot.revision.saturating_add(1);
        let candidate = match editor_ui_model::trusted_editor_localization_bundle()
            .snapshot(locale.clone(), next_revision)
        {
            Ok(candidate) => candidate,
            Err(diagnostic) => {
                self.localization_diagnostic = Some(diagnostic.clone());
                return EditorLocaleChangeResult {
                    changed: false,
                    snapshot: self.localization_snapshot.clone(),
                    diagnostic: Some(diagnostic),
                };
            }
        };
        let Some(store) = &self.editor_preference_store else {
            let diagnostic = EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::SwitchRejected,
                "Editor preference store is unavailable".to_string(),
            );
            self.localization_diagnostic = Some(diagnostic.clone());
            return EditorLocaleChangeResult {
                changed: false,
                snapshot: self.localization_snapshot.clone(),
                diagnostic: Some(diagnostic),
            };
        };
        let save = store.save(&EditorPreferencesDocument {
            schema_version: crate::EDITOR_PREFERENCES_SCHEMA_VERSION.to_string(),
            locale,
        });
        if !save.written {
            self.localization_diagnostic = save.diagnostic.clone();
            return EditorLocaleChangeResult {
                changed: false,
                snapshot: self.localization_snapshot.clone(),
                diagnostic: save.diagnostic,
            };
        }
        let changed = self.localization_snapshot.locale != candidate.locale;
        self.localization_snapshot = candidate.clone();
        self.localization_diagnostic = None;
        self.redraw_requested = true;
        EditorLocaleChangeResult {
            changed,
            snapshot: candidate,
            diagnostic: None,
        }
    }

    pub fn workspace_persistence_diagnostics(&self) -> &[WorkspacePersistenceDiagnostic] {
        &self.workspace_persistence_diagnostics
    }

    pub fn close_workspace_panel(&mut self, panel_id: &str) -> WorkspaceUpdate {
        self.workspace_panel_popup = None;
        let Some(panel_id) = PanelId::new(panel_id) else {
            return WorkspaceUpdate {
                changed: false,
                layout_revision: self
                    .workspace_docking
                    .snapshot(editor_workspace_rect(
                        self.latest_surface_width,
                        self.latest_surface_height,
                    ))
                    .layout_revision,
                diagnostics: Vec::new(),
            };
        };
        let update = self
            .workspace_docking
            .update(WorkspaceIntent::ClosePanel { panel_id });
        if update.changed {
            self.persist_workspace_layout();
            self.redraw_requested = true;
        }
        update
    }

    pub fn show_workspace_panel(&mut self, panel_id: &str) -> WorkspaceUpdate {
        let Some(panel_id) = PanelId::new(panel_id) else {
            return WorkspaceUpdate {
                changed: false,
                layout_revision: self
                    .workspace_docking
                    .snapshot(editor_workspace_rect(
                        self.latest_surface_width,
                        self.latest_surface_height,
                    ))
                    .layout_revision,
                diagnostics: Vec::new(),
            };
        };
        let update = self
            .workspace_docking
            .update(WorkspaceIntent::ShowPanel { panel_id });
        if update.changed {
            self.persist_workspace_layout();
            self.redraw_requested = true;
        }
        update
    }

    pub fn reset_workspace_layout(&mut self) -> WorkspaceUpdate {
        let update = self.workspace_docking.update(WorkspaceIntent::ResetLayout);
        if update.changed {
            self.persist_workspace_layout();
            self.redraw_requested = true;
        }
        update
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn close_floating_workspace_window(
        &mut self,
        window_id: editor_ui_renderer::WorkspaceWindowId,
    ) -> WorkspaceUpdate {
        let update = self
            .workspace_docking
            .update(WorkspaceIntent::CloseFloatingWindow { window_id });
        if update.changed {
            self.persist_workspace_layout();
            self.redraw_requested = true;
        }
        update
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn cancel_workspace_panel_drag(&mut self) {
        if self.workspace_docking.active_panel_drag_id().is_some() {
            self.workspace_docking
                .update(WorkspaceIntent::CancelPanelDrag);
            self.release_workspace_pointer_capture();
            self.redraw_requested = true;
        }
    }

    fn install_workspace_layout_store(&mut self, store: WorkspaceLayoutStore) {
        let load = store.load();
        let migrated_legacy = load.legacy_layout.is_some();
        let (workspace_docking, restore) =
            EditorWorkspaceDockingModule::restore_topology_or_default(
                PanelRegistry::standard_editor(),
                load.topology,
                load.legacy_layout,
            );
        self.workspace_docking = workspace_docking;
        self.workspace_persistence_diagnostics = load.diagnostics;
        self.workspace_persistence_diagnostics
            .extend(restore.diagnostics.into_iter().map(|diagnostic| {
                WorkspacePersistenceDiagnostic {
                    code: diagnostic.code,
                    path: store.path().display().to_string(),
                }
            }));
        if migrated_legacy {
            self.workspace_persistence_diagnostics
                .extend(store.save(&self.workspace_docking.topology()).diagnostics);
        }
        self.workspace_layout_store = Some(store);
        self.redraw_requested = true;
    }

    fn install_editor_preference_store(&mut self, store: EditorPreferenceStore) {
        let load = store.load();
        self.localization_snapshot = crate::snapshot_from_preferences(
            &load.preferences,
            self.localization_snapshot.revision,
        )
        .unwrap_or_default();
        self.localization_diagnostic = load.diagnostic;
        self.editor_preference_store = Some(store);
        self.redraw_requested = true;
    }

    fn persist_workspace_layout(&mut self) {
        let Some(store) = &self.workspace_layout_store else {
            return;
        };
        let save = store.save(&self.workspace_docking.topology());
        self.workspace_persistence_diagnostics
            .extend(save.diagnostics);
    }

    pub fn toolbar_overflow_open(&self) -> bool {
        self.toolbar_overflow_open
    }

    pub fn focus_input_mut(&mut self) -> &mut EditorFocusInputSystem {
        &mut self.focus_input
    }

    pub fn focus_input(&self) -> &EditorFocusInputSystem {
        &self.focus_input
    }

    pub fn widget_interaction_snapshot(&self) -> &editor_input::WidgetInteractionSnapshot {
        self.widget_interaction.snapshot()
    }

    pub fn command_system(&self) -> &EditorCommandSystem {
        &self.command_system
    }

    pub fn transaction_service(&self) -> &EditorTransactionService {
        &self.transaction_service
    }

    pub fn project_manager(&self) -> &ProjectManagerController {
        &self.project_manager
    }

    pub fn project_dialog_initial_directory(&self) -> &Path {
        &self.project_dialog_initial_directory
    }

    pub fn authoring_workspace(&self) -> &EditorAuthoringWorkspace {
        &self.authoring_workspace
    }

    pub fn main_frame(&self) -> &EditorMainFrame {
        &self.main_frame
    }

    pub fn config(&self) -> &NativeEditorWindowConfig {
        &self.config
    }

    pub fn report(&self) -> NativeEditorApplicationReport {
        let mut workspace = self.authoring_workspace.clone();
        workspace.set_panel_state(
            self.focus_input.active_panel_id.clone(),
            self.focus_input.active_panel_id.clone(),
            self.focus_input.hovered_panel_id.clone(),
        );
        let interaction = self.widget_interaction.snapshot();
        let tree = self.retained_ui_renderer.tree();
        let hit_id = |widget_id: Option<&editor_ui_renderer::WidgetId>| {
            widget_id
                .and_then(|id| tree.and_then(|tree| tree.node(id)))
                .and_then(|node| node.hit_region_id.clone())
        };
        NativeEditorApplicationReport {
            mode: self.latest_model.mode.clone(),
            frame_index: self.frame_index,
            model_revision: self.latest_model.revision,
            draw_command_count: self.latest_draw_list.commands.len(),
            hit_region_count: self.latest_draw_list.hit_regions.len(),
            panel_count: editor_ui_renderer::native_editor_panel_manifest().len(),
            command_count: self.command_system.count(),
            last_command_id: self.last_command_id.clone(),
            last_command_status: self.last_command_status,
            active_panel_id: self.focus_input.active_panel_id.clone(),
            hovered_panel_id: self.focus_input.hovered_panel_id.clone(),
            hovered_hit_id: hit_id(interaction.hovered_widget_id.as_ref())
                .or_else(|| self.focus_input.hovered_hit_id.clone()),
            pressed_hit_id: hit_id(interaction.active_widget_id.as_ref())
                .or_else(|| self.focus_input.pressed_hit_id.clone()),
            last_feedback: self.last_feedback.clone(),
            redraw_requested: self.redraw_requested,
            workspace: workspace.report(),
        }
    }
}

impl Drop for NativeEditorApplication {
    fn drop(&mut self) {
        self.cancel_project_runtime_worker_only();
        if let Some(mut worker) = self.editor_play_preparation_worker.take() {
            worker.cancelled.store(true, Ordering::Release);
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
        if let Some(mut worker) = self.project_open_preparation_worker.take() {
            worker.cancelled.store(true, Ordering::Release);
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
    }
}

fn project_open_path(payload: &UiCommandPayload) -> Option<&str> {
    match payload {
        UiCommandPayload::OpenProject { path } | UiCommandPayload::SelectRecentProject { path } => {
            Some(path)
        }
        _ => None,
    }
}

fn viewport_texture_content_rect(
    command: &DrawCommand,
    texture_id: &str,
    target_id: &str,
    inherited_clip: Option<editor_ui_renderer::UiRect>,
) -> Option<editor_ui_renderer::UiRect> {
    match command {
        DrawCommand::Clipped { clip, command } => {
            let clip = match inherited_clip {
                Some(inherited) => inherited.intersection(*clip)?,
                None => *clip,
            };
            viewport_texture_content_rect(command, texture_id, target_id, Some(clip))
        }
        DrawCommand::ViewportTextureSlot {
            rect,
            texture_id: Some(candidate_texture),
            target_id: Some(candidate_target),
            ..
        } if candidate_texture == texture_id && candidate_target == target_id => {
            inherited_clip.map_or(Some(*rect), |clip| rect.intersection(clip))
        }
        _ => None,
    }
}

fn image_texture_ids(draw_list: &UiDrawList) -> BTreeSet<String> {
    draw_list
        .commands
        .iter()
        .filter(|command| {
            command.clip().is_none_or(|clip| {
                let rect = match command.unclipped() {
                    editor_ui_renderer::DrawCommand::ImageTextureSlot { rect, .. } => *rect,
                    _ => return true,
                };
                rect.intersection(clip).is_some()
            })
        })
        .filter_map(|command| match command.unclipped() {
            editor_ui_renderer::DrawCommand::ImageTextureSlot {
                texture_id: Some(texture_id),
                ..
            } if texture_id.starts_with("asset-thumbnail::") => Some(texture_id.clone()),
            _ => None,
        })
        .collect()
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn project_runtime_trust_request_id(request: &ProjectRuntimeTrustRequest) -> String {
    let digest = request
        .runtime_module_source_digest
        .trim_start_matches("sha256:");
    format!(
        "project-runtime-trust-{}-{}",
        request.project_id,
        &digest[..digest.len().min(12)]
    )
}

fn native_host_result(
    command_id: &str,
    request_id: &str,
    status: CommandStatus,
    code: impl Into<String>,
    message: impl Into<String>,
) -> CommandResult {
    let severity = if matches!(status, CommandStatus::Failed | CommandStatus::Rejected) {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Info
    };
    CommandResult {
        transaction_id: format!("native-host-{request_id}"),
        request_id: request_id.to_string(),
        command_id: command_id.to_string(),
        status,
        diagnostics: vec![EditorDiagnostic {
            severity,
            code: code.into(),
            message: message.into(),
            source: DiagnosticSource::UiBackend,
            command_id: Some(command_id.to_string()),
            request_id: Some(request_id.to_string()),
            path: None,
            entity_id: None,
            trace_entry_id: None,
            suggested_action: None,
        }],
        console_entries: Vec::new(),
        state_changes: Vec::new(),
        ui_model_revision: 0,
    }
}

fn short_gateway_session_id(client_session_id: &str) -> String {
    let suffix = client_session_id
        .strip_prefix("gateway-session-")
        .unwrap_or(client_session_id);
    let chars = suffix.chars().collect::<Vec<_>>();
    chars[chars.len().saturating_sub(10)..].iter().collect()
}

fn gateway_client_kind_label(kind: ai_tool_gateway::ClientKind) -> &'static str {
    match kind {
        ai_tool_gateway::ClientKind::Mcp => "MCP",
        ai_tool_gateway::ClientKind::Cli => "CLI",
        ai_tool_gateway::ClientKind::NativeEditor => "Editor",
        ai_tool_gateway::ClientKind::Test => "Test",
    }
}
