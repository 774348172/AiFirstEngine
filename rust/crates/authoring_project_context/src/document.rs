use crate::authority_lock::ProjectAuthorityLock;
use crate::mutation::{
    mutation_error, read_optional_regular, validate_relative_path, write_project_file_atomic,
};
use crate::{CanonicalSourcePolicy, ContextError, ProjectRevision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const DOCUMENT_WRITE_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentWriteStatus {
    Saved,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentWriteRequest {
    pub relative_path: String,
    pub domain: String,
    pub schema_version: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentWriteReport {
    pub status: DocumentWriteStatus,
    pub relative_path: String,
    pub domain: String,
    pub schema_version: String,
    pub before_digest: Option<String>,
    pub after_digest: String,
    pub observed_revision: ProjectRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocumentWriteOutcome {
    pub status: DocumentWriteStatus,
    pub relative_path: String,
    pub domain: String,
    pub schema_version: String,
    pub before_digest: Option<String>,
    pub after_digest: String,
}

pub(crate) fn save_document(
    project_root: &Path,
    request: DocumentWriteRequest,
) -> Result<DocumentWriteOutcome, ContextError> {
    let relative_path = validate_relative_path(&request.relative_path)
        .map_err(|error| document_wrap(error, "authoring_context.document_path_invalid"))?;
    if !CanonicalSourcePolicy::allows_change(project_root, Path::new(&relative_path)) {
        return Err(document_error(
            "authoring_context.document_path_excluded",
            "Document path is outside the canonical source policy.",
            Some(relative_path),
            "Choose a canonical project source document.",
        ));
    }
    validate_identity(&request.domain, "domain")?;
    validate_identity(&request.schema_version, "schema version")?;
    if request.bytes.len() > DOCUMENT_WRITE_MAX_BYTES {
        return Err(document_error(
            "authoring_context.document_too_large",
            format!(
                "Document exceeds the {} byte limit.",
                DOCUMENT_WRITE_MAX_BYTES
            ),
            Some(relative_path),
            "Reduce the document size or use a structured asset mutation.",
        ));
    }

    let _authority = ProjectAuthorityLock::acquire(project_root, "document-save")?;
    let before = read_optional_regular(project_root, &relative_path)?;
    let before_digest = before.as_deref().map(bytes_digest);
    let after_digest = bytes_digest(&request.bytes);
    let status = if before.as_deref() == Some(request.bytes.as_slice()) {
        DocumentWriteStatus::Unchanged
    } else {
        write_project_file_atomic(project_root, &relative_path, &request.bytes)
            .map_err(|error| document_wrap(error, "authoring_context.document_write_failed"))?;
        DocumentWriteStatus::Saved
    };
    Ok(DocumentWriteOutcome {
        status,
        relative_path,
        domain: request.domain,
        schema_version: request.schema_version,
        before_digest,
        after_digest,
    })
}

fn validate_identity(value: &str, label: &str) -> Result<(), ContextError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(document_error(
            "authoring_context.document_identity_invalid",
            format!("Document {label} must use portable ASCII identity syntax."),
            None,
            "Use letters, digits, '.', '-' or '_'.",
        ));
    }
    Ok(())
}

fn bytes_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn document_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<String>,
    next_action: impl Into<String>,
) -> ContextError {
    mutation_error(code, message, path, next_action)
}

fn document_wrap(error: ContextError, code: &str) -> ContextError {
    ContextError::new(
        code,
        error.diagnostic.message,
        error.diagnostic.stage,
        error.diagnostic.path,
        error.diagnostic.next_action,
    )
}
