use crate::{
    CanonicalSourceInventory, CanonicalSourcePolicy, ContextError, DiagnosticStage,
    ProjectQualification, ProjectRevision,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

pub const PROJECT_SNAPSHOT_SCHEMA_VERSION: &str = "bounded-project-snapshot.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotLeaseId(u64);

impl SnapshotLeaseId {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotRetentionPolicy {
    OperationBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotLeaseRequest {
    pub owner: String,
    pub snapshot: SnapshotRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotReleaseOutcome {
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotReleaseReport {
    pub lease_id: SnapshotLeaseId,
    pub snapshot_id: String,
    pub revision_id: String,
    pub outcome: SnapshotReleaseOutcome,
}

#[derive(Debug)]
pub struct ProjectSnapshotLease {
    lease_id: SnapshotLeaseId,
    owner: String,
    retention_policy: SnapshotRetentionPolicy,
    snapshot: Option<ProjectSnapshot>,
}

impl ProjectSnapshotLease {
    pub(crate) fn new(lease_id: SnapshotLeaseId, owner: String, snapshot: ProjectSnapshot) -> Self {
        Self {
            lease_id,
            owner,
            retention_policy: SnapshotRetentionPolicy::OperationBound,
            snapshot: Some(snapshot),
        }
    }

    pub fn lease_id(&self) -> SnapshotLeaseId {
        self.lease_id
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn retention_policy(&self) -> SnapshotRetentionPolicy {
        self.retention_policy
    }

    pub fn snapshot(&self) -> &ProjectSnapshot {
        self.snapshot
            .as_ref()
            .expect("an unreleased lease always owns its snapshot")
    }

    pub fn release(mut self) -> SnapshotReleaseReport {
        let snapshot = self
            .snapshot
            .take()
            .expect("an unreleased lease always owns its snapshot");
        SnapshotReleaseReport {
            lease_id: self.lease_id,
            snapshot_id: snapshot.snapshot_id,
            revision_id: snapshot.revision.revision_id,
            outcome: SnapshotReleaseOutcome::Released,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotRequest {
    pub relative_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotFile {
    pub relative_path: String,
    pub length: u64,
    pub content_digest: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub schema_version: String,
    pub snapshot_id: String,
    pub portable_project_identity: String,
    pub opened_project_binding: String,
    pub revision: ProjectRevision,
    pub qualification: ProjectQualification,
    pub files: Vec<SnapshotFile>,
}

pub(crate) fn capture_project_snapshot(
    project_id: &str,
    root_binding: &[u8; 32],
    revision: &ProjectRevision,
    inventory: &CanonicalSourceInventory,
    request: SnapshotRequest,
) -> Result<ProjectSnapshot, ContextError> {
    let mut requested_paths = request
        .relative_paths
        .into_iter()
        .map(|path| canonical_request_path(&path))
        .collect::<Result<Vec<_>, _>>()?;
    requested_paths.sort();
    let mut unique_paths = BTreeSet::new();
    for path in &requested_paths {
        if !unique_paths.insert(path.clone()) {
            return Err(snapshot_error(
                "authoring_context.snapshot_path_invalid",
                "Snapshot scope contains the same canonical path more than once.",
                Some(path),
                "Request each canonical source path once.",
            ));
        }
    }

    let mut files = Vec::with_capacity(requested_paths.len());
    for relative_path in requested_paths {
        let path = Path::new(&relative_path);
        if !CanonicalSourcePolicy::allows_change(inventory.project_root(), path) {
            return Err(snapshot_error(
                "authoring_context.snapshot_path_excluded",
                "Snapshot scope targets a generated or excluded project path.",
                Some(&relative_path),
                "Request a canonical project source file.",
            ));
        }
        let Some(entry) = inventory.entry(&relative_path) else {
            let live_path = inventory.project_root().join(path);
            let diagnostic = match fs::symlink_metadata(&live_path) {
                Ok(metadata) if metadata.is_dir() || !metadata.is_file() => snapshot_error(
                    "authoring_context.snapshot_path_unsupported",
                    "Snapshot scope only supports canonical regular source files.",
                    Some(&relative_path),
                    "Request an individual canonical source file.",
                ),
                Ok(_) => snapshot_error(
                    "authoring_context.snapshot_source_changed",
                    "A requested source path is not part of the bound project revision.",
                    Some(&relative_path),
                    "Refresh the project and request a snapshot of the new revision.",
                ),
                Err(_) => snapshot_error(
                    "authoring_context.snapshot_path_missing",
                    "Snapshot scope targets a source path that does not exist.",
                    Some(&relative_path),
                    "Request a source file present in the bound project revision.",
                ),
            };
            return Err(diagnostic);
        };
        let bytes = inventory.read_verified(entry).map_err(|error| {
            snapshot_error(
                "authoring_context.snapshot_source_changed",
                error.diagnostic.message,
                Some(&relative_path),
                "Refresh the project and request a snapshot of the new revision.",
            )
        })?;
        files.push(SnapshotFile {
            relative_path,
            length: entry.length,
            content_digest: entry.content_digest.clone(),
            bytes,
        });
    }

    let opened_project_binding = format!("sha256:{}", hex_digest(root_binding));
    let snapshot_id = snapshot_id(
        project_id,
        &opened_project_binding,
        &revision.revision_id,
        &files,
    );
    Ok(ProjectSnapshot {
        schema_version: PROJECT_SNAPSHOT_SCHEMA_VERSION.to_string(),
        snapshot_id,
        portable_project_identity: project_id.to_string(),
        opened_project_binding,
        revision: revision.clone(),
        qualification: revision.qualification,
        files,
    })
}

fn canonical_request_path(path: &str) -> Result<String, ContextError> {
    if path.is_empty() || path.contains('\\') {
        return Err(snapshot_path_invalid(path));
    }
    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        let Component::Normal(value) = component else {
            return Err(snapshot_path_invalid(path));
        };
        let Some(value) = value.to_str() else {
            return Err(snapshot_path_invalid(path));
        };
        if value.is_empty() || value.ends_with([' ', '.']) {
            return Err(snapshot_path_invalid(path));
        }
        parts.push(value);
    }
    if parts.is_empty() {
        return Err(snapshot_path_invalid(path));
    }
    Ok(parts.join("/"))
}

fn snapshot_path_invalid(path: &str) -> ContextError {
    snapshot_error(
        "authoring_context.snapshot_path_invalid",
        "Snapshot scope path is not canonical project-relative syntax.",
        Some(path),
        "Use a canonical UTF-8 project-relative file path.",
    )
}

fn snapshot_id(
    project_id: &str,
    opened_project_binding: &str,
    revision_id: &str,
    files: &[SnapshotFile],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_SNAPSHOT_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(project_id.as_bytes());
    hasher.update([0]);
    hasher.update(opened_project_binding.as_bytes());
    hasher.update([0]);
    hasher.update(revision_id.as_bytes());
    hasher.update([0]);
    for file in files {
        hasher.update((file.relative_path.len() as u64).to_le_bytes());
        hasher.update(file.relative_path.as_bytes());
        hasher.update(file.length.to_le_bytes());
        hasher.update(file.content_digest.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn snapshot_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<&str>,
    next_action: impl Into<String>,
) -> ContextError {
    ContextError::new(
        code,
        message,
        DiagnosticStage::Snapshot,
        path.map(ToOwned::to_owned),
        next_action,
    )
}
