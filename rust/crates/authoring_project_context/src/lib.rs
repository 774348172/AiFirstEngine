mod authority_lock;
mod context;
mod diagnostic;
mod document;
mod mutation;
mod snapshot;
mod source_inventory;

pub use context::{
    EmbeddedAuthoringProjectContext, OpenOptions, OpenReport, ProjectHandle, ProjectLocator,
    ProjectQualification, ProjectRevision, RefreshReport, RefreshStatus,
};
pub use diagnostic::{ContextError, DiagnosticStage, ProjectDiagnostic};
pub use document::{
    DocumentWriteReport, DocumentWriteRequest, DocumentWriteStatus, DOCUMENT_WRITE_MAX_BYTES,
};
pub use mutation::{
    MutationCommitProgress, ProjectMutation, ProjectMutationBeforeState, ProjectMutationOperation,
    ProjectMutationReceipt, ProjectMutationRollbackReceipt, ProjectRecoveryDisposition,
    ProjectRecoveryProgress, ProjectRecoveryReport, ProjectRecoveryTransactionReport,
    PROJECT_MUTATION_RECEIPT_SCHEMA_VERSION, PROJECT_MUTATION_ROLLBACK_RECEIPT_SCHEMA_VERSION,
    PROJECT_MUTATION_SCHEMA_VERSION,
};
pub use snapshot::{
    ProjectSnapshot, ProjectSnapshotLease, SnapshotFile, SnapshotLeaseId, SnapshotLeaseRequest,
    SnapshotReleaseOutcome, SnapshotReleaseReport, SnapshotRequest, SnapshotRetentionPolicy,
    PROJECT_SNAPSHOT_SCHEMA_VERSION,
};
pub use source_inventory::{
    legacy_project_digest, legacy_project_digest_cancellable, CanonicalSourceInventory,
    CanonicalSourcePolicy, SourceInventoryEntry, SOURCE_POLICY_VERSION,
};

#[cfg(test)]
mod tests;
