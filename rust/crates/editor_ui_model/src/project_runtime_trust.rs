use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTrustPromptModel {
    pub request_id: String,
    pub project_name: String,
    pub canonical_project_root: String,
    pub module_id: String,
    pub dependency_summary: Vec<String>,
    pub identity_changed: bool,
}
