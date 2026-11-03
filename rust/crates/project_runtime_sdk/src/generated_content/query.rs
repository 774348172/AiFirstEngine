//! Compact semantic queries carried by the existing world-query callback.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "query",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ContentQuery {
    Request { request_id: u64 },
    Ticket { ticket: ContentTicket },
    Target { target: ProjectRuntimeCellEntity },
}
impl ContentQuery {
    pub fn validate(&self) -> Result<(), ContentError> {
        let valid = match self {
            Self::Request { request_id } => *request_id != 0,
            Self::Ticket { ticket } => ticket.request_id != 0 && ticket.session_generation != 0,
            Self::Target { target } => {
                target.generation != 0
                    && !target.entity_id.trim().is_empty()
                    && target.entity_id.len() <= 256
            }
        };
        valid
            .then_some(())
            .ok_or_else(|| ContentError::invalid("generatedContent.query"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentQueryRequest {
    pub generated_content: ContentQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentTargetState {
    pub session_generation: u64,
    pub target: ProjectRuntimeCellEntity,
    pub applied_revision: u64,
    pub pending_revision: Option<u64>,
    pub collision_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentQueryValue {
    Receipt { receipt: ContentReceipt },
    Target { state: ContentTargetState },
}

/// Semantic errors remain typed; transport errors use the existing SDK/ABI status.
pub type ContentQueryReply = Result<ContentQueryValue, ContentError>;
