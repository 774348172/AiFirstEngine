//! Opaque bytes. The host selects the project/profile namespace, never the project path.
use crate::generated_content::{ContentError, ContentQuery, ContentTicket};
use serde::{Deserialize, Serialize};
mod read_wire;
pub use read_wire::*;
pub const PROJECT_STORAGE_SCHEMA: &str = "project-storage.v1;opaque-bytes;stable-host-namespace;read-reply=AIFEPB01;metadata+payload<=8388608;claim-after-copy;explicit-abandon";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "query",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StorageQuery {
    Request { request_id: u64 },
    Ticket { ticket: ContentTicket },
}
impl StorageQuery {
    pub fn validate(&self) -> Result<(), ContentError> {
        match self {
            Self::Request { request_id } => ContentQuery::Request {
                request_id: *request_id,
            },
            Self::Ticket { ticket } => ContentQuery::Ticket { ticket: *ticket },
        }
        .validate()
        .map_err(|mut error| {
            error.field_path = "storage.query".into();
            error
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageQueryRequest {
    pub project_storage: StorageQuery,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageTakeRequest {
    pub project_storage_take: ContentTicket,
}
pub type StorageQueryReply = Result<StorageReceipt, ContentError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum StorageOperation {
    Read {
        key: String,
    },
    Write {
        key: String,
        #[serde(skip)]
        bytes: Vec<u8>,
    },
    Delete {
        key: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectStorageRequest {
    pub request_id: u64,
    pub operation: StorageOperation,
}
impl ProjectStorageRequest {
    pub fn validate(&self) -> Result<(), ContentError> {
        let key = match &self.operation {
            StorageOperation::Read { key }
            | StorageOperation::Write { key, .. }
            | StorageOperation::Delete { key } => key,
        };
        if self.request_id == 0
            || key.is_empty()
            || key.len() > 256
            || key.split('/').any(|c| {
                c.is_empty() || c == "." || c == ".." || c.ends_with('.') || c.ends_with(' ')
            })
            || key.chars().any(|c| {
                c.is_control() || matches!(c, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            })
        {
            let mut e = ContentError::invalid("storage.key");
            e.request_id = self.request_id;
            return Err(e);
        }
        Ok(())
    }
    pub(crate) fn payloads_mut(&mut self, mut f: impl FnMut(&mut Vec<u8>)) {
        if let StorageOperation::Write { bytes, .. } = &mut self.operation {
            f(bytes);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageCommitState {
    NotCommitted,
    Committed,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageReceipt {
    pub ticket: crate::generated_content::ContentTicket,
    pub status: crate::generated_content::ContentStatus,
    pub commit_state: StorageCommitState,
    pub byte_count: u64,
    pub result_claimed: bool,
    pub error: Option<ContentError>,
}
