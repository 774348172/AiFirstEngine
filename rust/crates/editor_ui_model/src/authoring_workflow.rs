use serde::{Deserialize, Serialize};

use super::{ManualWalkthroughCoverageSummary, UiCommandSource, WorkspaceDomainKind};

pub const AUTHORING_WORKFLOW_SCHEMA_VERSION: &str = "authoring-workflow.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringWorkflowModel {
    pub schema_version: String,
    pub project_id: Option<String>,
    pub active_step: AuthoringStepId,
    pub steps: Vec<AuthoringWorkflowStep>,
    pub global_status: AuthoringStepStatus,
    pub can_play: bool,
    pub can_build: bool,
    pub blocking_issues: Vec<AuthoringIssue>,
    pub recommended_tasks: Vec<AuthoringTask>,
    pub ai_context: AuthoringAiContext,
}

impl AuthoringWorkflowModel {
    pub fn empty() -> Self {
        let mut steps = AuthoringStepId::all()
            .into_iter()
            .map(AuthoringWorkflowStep::not_available)
            .collect::<Vec<_>>();

        if let Some(project_step) = steps
            .iter_mut()
            .find(|step| step.id == AuthoringStepId::Project)
        {
            project_step.status = AuthoringStepStatus::Ready;
            project_step.completion = AuthoringStepCompletion::Ready;
            project_step.next_hint = Some("Open or create a project.".to_string());
            project_step.primary_command = Some(AuthoringCommand::new(
                "open_project",
                WorkspaceDomainKind::Project,
                "Open Project",
                AuthoringCommandAvailability::Available,
                "OpenProject",
            ));
        }

        let recommended_tasks = vec![AuthoringTask::new(
            "open_or_create_project",
            WorkspaceDomainKind::Project,
            AuthoringTaskPriority::Critical,
            "Open or create a project",
            "No project is open.",
            Some(AuthoringCommand::new(
                "open_project",
                WorkspaceDomainKind::Project,
                "Open Project",
                AuthoringCommandAvailability::Available,
                "OpenProject",
            )),
        )];

        Self {
            schema_version: AUTHORING_WORKFLOW_SCHEMA_VERSION.to_string(),
            project_id: None,
            active_step: AuthoringStepId::Project,
            steps,
            global_status: AuthoringStepStatus::Blocked,
            can_play: false,
            can_build: false,
            blocking_issues: Vec::new(),
            recommended_tasks: recommended_tasks.clone(),
            ai_context: AuthoringAiContext {
                active_step: AuthoringStepId::Project,
                missing_required_items: vec!["project".to_string()],
                blocking_issues: Vec::new(),
                recommended_tasks,
                available_commands: vec![AuthoringCommand::new(
                    "open_project",
                    WorkspaceDomainKind::Project,
                    "Open Project",
                    AuthoringCommandAvailability::Available,
                    "OpenProject",
                )],
                manual_walkthrough_coverage: None,
                project_patch_summary: None,
                prefab_authoring_summary: None,
                aui_authoring_summary: Some(AuiAuthoringAiContextSummary::not_productized()),
                summary: "Open or create a project to start authoring.".to_string(),
            },
        }
    }

    pub fn step(&self, id: AuthoringStepId) -> Option<&AuthoringWorkflowStep> {
        self.steps.iter().find(|step| step.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringWorkflowStep {
    pub id: AuthoringStepId,
    pub domain: WorkspaceDomainKind,
    pub title: String,
    pub status: AuthoringStepStatus,
    pub completion: AuthoringStepCompletion,
    pub item_count: usize,
    pub is_required_for_play: bool,
    pub is_required_for_build: bool,
    pub primary_command: Option<AuthoringCommand>,
    pub secondary_commands: Vec<AuthoringCommand>,
    pub issues: Vec<AuthoringIssue>,
    pub next_hint: Option<String>,
}

impl AuthoringWorkflowStep {
    pub fn new(
        id: AuthoringStepId,
        status: AuthoringStepStatus,
        completion: AuthoringStepCompletion,
    ) -> Self {
        Self {
            id,
            domain: id.domain(),
            title: id.label().to_string(),
            status,
            completion,
            item_count: 0,
            is_required_for_play: id.is_required_for_play(),
            is_required_for_build: id.is_required_for_build(),
            primary_command: None,
            secondary_commands: Vec::new(),
            issues: Vec::new(),
            next_hint: None,
        }
    }

    pub fn not_available(id: AuthoringStepId) -> Self {
        Self::new(
            id,
            AuthoringStepStatus::NotAvailable,
            AuthoringStepCompletion::Blocked,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringStepId {
    Project,
    Assets,
    Scene,
    Prefabs,
    Rules,
    Input,
    Aui,
    Play,
    Build,
    Reports,
}

impl AuthoringStepId {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Project,
            Self::Assets,
            Self::Scene,
            Self::Prefabs,
            Self::Rules,
            Self::Input,
            Self::Aui,
            Self::Play,
            Self::Build,
            Self::Reports,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Assets => "assets",
            Self::Scene => "scene",
            Self::Prefabs => "prefabs",
            Self::Rules => "rules",
            Self::Input => "input",
            Self::Aui => "aui",
            Self::Play => "play",
            Self::Build => "build",
            Self::Reports => "reports",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Assets => "Assets",
            Self::Scene => "Scene",
            Self::Prefabs => "Prefabs",
            Self::Rules => "Rules",
            Self::Input => "Input",
            Self::Aui => "AUI",
            Self::Play => "Play",
            Self::Build => "Build",
            Self::Reports => "Reports",
        }
    }

    pub fn domain(&self) -> WorkspaceDomainKind {
        match self {
            Self::Project => WorkspaceDomainKind::Project,
            Self::Assets => WorkspaceDomainKind::Asset,
            Self::Scene => WorkspaceDomainKind::Scene,
            Self::Prefabs => WorkspaceDomainKind::Prefab,
            Self::Rules => WorkspaceDomainKind::Rule,
            Self::Input => WorkspaceDomainKind::Input,
            Self::Aui => WorkspaceDomainKind::Aui,
            Self::Play => WorkspaceDomainKind::Play,
            Self::Build => WorkspaceDomainKind::Build,
            Self::Reports => WorkspaceDomainKind::Report,
        }
    }

    pub fn is_required_for_play(&self) -> bool {
        matches!(self, Self::Project | Self::Scene | Self::Rules)
    }

    pub fn is_required_for_build(&self) -> bool {
        matches!(
            self,
            Self::Project | Self::Assets | Self::Scene | Self::Rules | Self::Build
        )
    }
}

impl std::str::FromStr for AuthoringStepId {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "project" => Ok(Self::Project),
            "assets" => Ok(Self::Assets),
            "scene" => Ok(Self::Scene),
            "prefabs" => Ok(Self::Prefabs),
            "rules" => Ok(Self::Rules),
            "input" => Ok(Self::Input),
            "aui" => Ok(Self::Aui),
            "play" => Ok(Self::Play),
            "build" => Ok(Self::Build),
            "reports" => Ok(Self::Reports),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringStepStatus {
    NotAvailable,
    Empty,
    NeedsAttention,
    Ready,
    Dirty,
    Running,
    Blocked,
    Failed,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringStepCompletion {
    Missing,
    Partial,
    Ready,
    Blocked,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringTask {
    pub id: String,
    pub domain: WorkspaceDomainKind,
    pub priority: AuthoringTaskPriority,
    pub title: String,
    pub reason: String,
    pub command: Option<AuthoringCommand>,
    pub is_ai_actionable: bool,
    pub is_user_actionable: bool,
}

impl AuthoringTask {
    pub fn new(
        id: impl Into<String>,
        domain: WorkspaceDomainKind,
        priority: AuthoringTaskPriority,
        title: impl Into<String>,
        reason: impl Into<String>,
        command: Option<AuthoringCommand>,
    ) -> Self {
        Self {
            id: id.into(),
            domain,
            priority,
            title: title.into(),
            reason: reason.into(),
            command,
            is_ai_actionable: true,
            is_user_actionable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringTaskPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringIssue {
    pub id: String,
    pub domain: WorkspaceDomainKind,
    pub severity: AuthoringIssueSeverity,
    pub message: String,
    pub source_ref: Option<String>,
    pub blocks_play: bool,
    pub blocks_build: bool,
    pub suggested_command: Option<AuthoringCommand>,
}

impl AuthoringIssue {
    pub fn new(
        id: impl Into<String>,
        domain: WorkspaceDomainKind,
        severity: AuthoringIssueSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            domain,
            severity,
            message: message.into(),
            source_ref: None,
            blocks_play: false,
            blocks_build: false,
            suggested_command: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringIssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringCommand {
    pub command_id: String,
    pub source: UiCommandSource,
    pub domain: WorkspaceDomainKind,
    pub label: String,
    pub availability: AuthoringCommandAvailability,
    pub payload_kind: String,
}

impl AuthoringCommand {
    pub fn new(
        command_id: impl Into<String>,
        domain: WorkspaceDomainKind,
        label: impl Into<String>,
        availability: AuthoringCommandAvailability,
        payload_kind: impl Into<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            source: UiCommandSource::Toolbar,
            domain,
            label: label.into(),
            availability,
            payload_kind: payload_kind.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthoringCommandAvailability {
    Available,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringAiContext {
    pub active_step: AuthoringStepId,
    pub missing_required_items: Vec<String>,
    pub blocking_issues: Vec<AuthoringIssue>,
    pub recommended_tasks: Vec<AuthoringTask>,
    pub available_commands: Vec<AuthoringCommand>,
    pub manual_walkthrough_coverage: Option<ManualWalkthroughCoverageSummary>,
    pub project_patch_summary: Option<ProjectPatchAiContextSummary>,
    pub prefab_authoring_summary: Option<PrefabAuthoringAiContextSummary>,
    pub aui_authoring_summary: Option<AuiAuthoringAiContextSummary>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPatchAiContextSummary {
    pub productized: bool,
    pub imported_patch_productized: bool,
    pub llm_patch_source_available: bool,
    pub active_patch_source_kind: String,
    pub supported_capabilities: Vec<String>,
    pub unsupported_capabilities: Vec<String>,
    pub supported_import_sources: Vec<String>,
    pub imported_patch_commands: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabAuthoringAiContextSummary {
    pub productized: bool,
    pub supported_commands: Vec<String>,
    pub deferred_capabilities: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiAuthoringAiContextSummary {
    pub productized: bool,
    pub scene_unified_authoring: bool,
    pub visual_order_runtime_supported: bool,
    pub visual_order_runtime_support_reason: String,
    pub runtime_composition_gap_count: usize,
    pub next_required_runtime_gate: Option<String>,
    pub supported_commands: Vec<String>,
    pub deferred_capabilities: Vec<String>,
    pub next_actions: Vec<String>,
}

impl AuiAuthoringAiContextSummary {
    pub fn not_productized() -> Self {
        Self {
            productized: false,
            scene_unified_authoring: false,
            visual_order_runtime_supported: true,
            visual_order_runtime_support_reason: "aui_scene_unified_authoring_not_loaded"
                .to_string(),
            runtime_composition_gap_count: 0,
            next_required_runtime_gate: None,
            supported_commands: Vec::new(),
            deferred_capabilities: vec!["aui_scene_unified_authoring".to_string()],
            next_actions: vec!["AUI Scene Unified Authoring Productization v1".to_string()],
        }
    }
}
