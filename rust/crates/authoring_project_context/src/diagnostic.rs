use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStage {
    Open,
    Refresh,
    Snapshot,
    Mutation,
    Rollback,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiagnostic {
    pub code: String,
    pub message: String,
    pub stage: DiagnosticStage,
    pub path: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextError {
    pub diagnostic: ProjectDiagnostic,
}

impl ContextError {
    pub(crate) fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        stage: DiagnosticStage,
        path: Option<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            diagnostic: ProjectDiagnostic {
                code: code.into(),
                message: message.into(),
                stage,
                path,
                next_action: next_action.into(),
            },
        }
    }
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.diagnostic.code, self.diagnostic.message
        )
    }
}

impl std::error::Error for ContextError {}
