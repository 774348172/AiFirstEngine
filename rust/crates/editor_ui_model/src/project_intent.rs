use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectIntentReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIntentWorkItemModel {
    pub work_item_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub ready: bool,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIntentModel {
    pub pre_project_draft_active: bool,
    pub journal_revision: u64,
    pub active_count: usize,
    pub parked_count: usize,
    pub needs_evidence_count: usize,
    pub pending_normalization_count: usize,
    pub work_items: Vec<ProjectIntentWorkItemModel>,
    pub latest_summary: Option<String>,
}

impl ProjectIntentModel {
    pub fn empty() -> Self {
        Self {
            pre_project_draft_active: false,
            journal_revision: 0,
            active_count: 0,
            parked_count: 0,
            needs_evidence_count: 0,
            pending_normalization_count: 0,
            work_items: Vec::new(),
            latest_summary: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectChangeReviewModel {
    pub proposal_id: Option<String>,
    pub proposal_digest: Option<String>,
    pub selected_work_item_count: usize,
    pub user_visible_outcomes: Vec<String>,
    pub explicit_exclusions: Vec<String>,
    pub risks: Vec<String>,
    pub required_decisions: Vec<String>,
    pub approval_ready: bool,
}

impl ProjectChangeReviewModel {
    pub fn empty() -> Self {
        Self {
            proposal_id: None,
            proposal_digest: None,
            selected_work_item_count: 0,
            user_visible_outcomes: Vec::new(),
            explicit_exclusions: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            approval_ready: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProductionModel {
    pub run_id: Option<String>,
    pub state: Option<String>,
    pub active_step_id: Option<String>,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub waiting_reason: Option<String>,
    pub recovery_options: Vec<String>,
    pub latest_result: Option<String>,
}

impl ProjectProductionModel {
    pub fn empty() -> Self {
        Self {
            run_id: None,
            state: None,
            active_step_id: None,
            completed_steps: 0,
            total_steps: 0,
            waiting_reason: None,
            recovery_options: Vec::new(),
            latest_result: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIntentWorkspaceModel {
    pub report_level: ProjectIntentReportLevel,
    pub intent: ProjectIntentModel,
    pub change_review: ProjectChangeReviewModel,
    pub production: ProjectProductionModel,
}

impl ProjectIntentWorkspaceModel {
    pub fn empty() -> Self {
        Self {
            report_level: ProjectIntentReportLevel::Summary,
            intent: ProjectIntentModel::empty(),
            change_review: ProjectChangeReviewModel::empty(),
            production: ProjectProductionModel::empty(),
        }
    }
}
