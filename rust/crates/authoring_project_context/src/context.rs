use crate::{
    CanonicalSourceInventory, ContextError, DiagnosticStage, ProjectDiagnostic,
    SOURCE_POLICY_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_MANIFEST_PATH: &str = "project.aife.json";
const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v2";
const PROJECT_REVISION_SCHEMA_VERSION: &str = "authoring-project-revision.v1";
const PROJECT_DIAGNOSTICS_SCHEMA_VERSION: &str = "authoring-project-diagnostics.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLocator {
    project_root: PathBuf,
}

impl ProjectLocator {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectHandle(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectQualification {
    Ready,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRevision {
    pub schema_version: String,
    pub portable_project_identity: String,
    pub source_policy_version: String,
    pub source_digest: String,
    pub revision_id: String,
    pub qualification: ProjectQualification,
    pub diagnostics_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStatus {
    Unchanged,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshReport {
    pub status: RefreshStatus,
    pub revision: ProjectRevision,
    pub diagnostics: Vec<ProjectDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReport {
    pub handle: ProjectHandle,
    pub recovery: crate::ProjectRecoveryReport,
}

#[derive(Debug)]
struct OpenProjectState {
    project_root: PathBuf,
    project_id: String,
    root_binding: [u8; 32],
    current_inventory: Option<CanonicalSourceInventory>,
    current_revision: Option<ProjectRevision>,
}

#[derive(Debug, Deserialize)]
struct ProjectManifestHeader {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    #[serde(rename = "projectId")]
    project_id: String,
}

#[derive(Debug, Default)]
pub struct EmbeddedAuthoringProjectContext {
    next_handle: u64,
    next_snapshot_lease: u64,
    projects: HashMap<ProjectHandle, OpenProjectState>,
}

impl EmbeddedAuthoringProjectContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(
        &mut self,
        locator: ProjectLocator,
        options: OpenOptions,
    ) -> Result<ProjectHandle, ContextError> {
        self.open_with_report(locator, options)
            .map(|report| report.handle)
    }

    pub fn open_with_report(
        &mut self,
        locator: ProjectLocator,
        _options: OpenOptions,
    ) -> Result<OpenReport, ContextError> {
        reject_link_or_reparse(locator.project_root(), DiagnosticStage::Open)?;
        let project_root = locator.project_root().canonicalize().map_err(|error| {
            ContextError::new(
                "authoring_context.root_unavailable",
                format!("Project root cannot be canonicalized: {error}"),
                DiagnosticStage::Open,
                Some(locator.project_root().display().to_string()),
                "Choose an existing project directory and retry.",
            )
        })?;
        if !project_root.is_dir() {
            return Err(ContextError::new(
                "authoring_context.root_not_directory",
                "Project root is not a directory.",
                DiagnosticStage::Open,
                Some(project_root.display().to_string()),
                "Choose a project directory and retry.",
            ));
        }

        let recovery = crate::mutation::recover_project(&project_root)?;

        let manifest_path = project_root.join(PROJECT_MANIFEST_PATH);
        reject_link_or_reparse(&manifest_path, DiagnosticStage::Open)?;
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            ContextError::new(
                "authoring_context.manifest_unreadable",
                format!("Project manifest cannot be read: {error}"),
                DiagnosticStage::Open,
                Some(manifest_path.display().to_string()),
                "Restore a readable project.aife.json and retry.",
            )
        })?;
        let manifest =
            serde_json::from_slice::<ProjectManifestHeader>(&manifest_bytes).map_err(|error| {
                ContextError::new(
                    "authoring_context.manifest_invalid",
                    format!("Project manifest cannot be parsed: {error}"),
                    DiagnosticStage::Open,
                    Some(manifest_path.display().to_string()),
                    "Repair project.aife.json and retry.",
                )
            })?;
        if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
            return Err(ContextError::new(
                "authoring_context.manifest_schema_unsupported",
                format!(
                    "Unsupported project manifest schema: {}",
                    manifest.schema_version
                ),
                DiagnosticStage::Open,
                Some(manifest_path.display().to_string()),
                format!("Migrate the manifest to {PROJECT_MANIFEST_SCHEMA_VERSION}."),
            ));
        }
        validate_project_id(&manifest.project_id, &manifest_path, DiagnosticStage::Open)?;

        let handle = ProjectHandle(self.next_handle.max(1));
        self.next_handle = handle.0.saturating_add(1);
        let root_binding = root_binding(&project_root, &manifest.project_id);
        self.projects.insert(
            handle,
            OpenProjectState {
                project_root,
                project_id: manifest.project_id,
                root_binding,
                current_inventory: None,
                current_revision: None,
            },
        );
        if let Err(error) = self.refresh(handle) {
            self.projects.remove(&handle);
            return Err(error);
        }
        Ok(OpenReport { handle, recovery })
    }

    pub fn refresh(&mut self, handle: ProjectHandle) -> Result<RefreshReport, ContextError> {
        let state = self.projects.get_mut(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Refresh,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        let inventory = CanonicalSourceInventory::capture(&state.project_root)?;
        let (revision, diagnostics) = revision_for_inventory(&state.project_id, &inventory);
        let status = if state.current_revision.as_ref() == Some(&revision) {
            RefreshStatus::Unchanged
        } else {
            RefreshStatus::Changed
        };
        state.current_inventory = Some(inventory);
        state.current_revision = Some(revision.clone());
        Ok(RefreshReport {
            status,
            revision,
            diagnostics,
        })
    }

    pub fn snapshot(
        &self,
        handle: ProjectHandle,
        request: crate::SnapshotRequest,
    ) -> Result<crate::ProjectSnapshot, ContextError> {
        let state = self.projects.get(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Snapshot,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        let inventory = state.current_inventory.as_ref().ok_or_else(|| {
            ContextError::new(
                "authoring_context.snapshot_revision_unavailable",
                "The opened project does not have a published source inventory.",
                DiagnosticStage::Snapshot,
                None,
                "Refresh the opened project before requesting a snapshot.",
            )
        })?;
        let revision = state.current_revision.as_ref().ok_or_else(|| {
            ContextError::new(
                "authoring_context.snapshot_revision_unavailable",
                "The opened project does not have a published revision.",
                DiagnosticStage::Snapshot,
                None,
                "Refresh the opened project before requesting a snapshot.",
            )
        })?;
        crate::snapshot::capture_project_snapshot(
            &state.project_id,
            &state.root_binding,
            revision,
            inventory,
            request,
        )
    }

    pub fn save_document(
        &mut self,
        handle: ProjectHandle,
        request: crate::DocumentWriteRequest,
    ) -> Result<crate::DocumentWriteReport, ContextError> {
        let project_root = self
            .projects
            .get(&handle)
            .ok_or_else(|| {
                ContextError::new(
                    "authoring_context.handle_invalid",
                    "Project handle is not open in this process.",
                    DiagnosticStage::Mutation,
                    None,
                    "Open the project and use the returned handle.",
                )
            })?
            .project_root
            .clone();
        let outcome = crate::document::save_document(&project_root, request)?;
        let refresh = self.refresh(handle)?;
        Ok(crate::DocumentWriteReport {
            status: outcome.status,
            relative_path: outcome.relative_path,
            domain: outcome.domain,
            schema_version: outcome.schema_version,
            before_digest: outcome.before_digest,
            after_digest: outcome.after_digest,
            observed_revision: refresh.revision,
        })
    }

    pub fn acquire_snapshot_lease(
        &mut self,
        handle: ProjectHandle,
        request: crate::SnapshotLeaseRequest,
    ) -> Result<crate::ProjectSnapshotLease, ContextError> {
        let crate::SnapshotLeaseRequest { owner, snapshot } = request;
        let snapshot = self.snapshot(handle, snapshot)?;
        let lease_id = self.next_snapshot_lease.checked_add(1).ok_or_else(|| {
            ContextError::new(
                "authoring_context.snapshot_lease_id_exhausted",
                "No process-local snapshot lease identity remains available.",
                DiagnosticStage::Snapshot,
                None,
                "Restart the Engine Provider process before acquiring another snapshot lease.",
            )
        })?;
        self.next_snapshot_lease = lease_id;
        Ok(crate::ProjectSnapshotLease::new(
            crate::SnapshotLeaseId::new(lease_id),
            owner,
            snapshot,
        ))
    }

    pub fn current_revision(&self, handle: ProjectHandle) -> Result<ProjectRevision, ContextError> {
        self.projects
            .get(&handle)
            .and_then(|state| state.current_revision.clone())
            .ok_or_else(|| {
                ContextError::new(
                    "authoring_context.mutation_revision_unavailable",
                    "The opened project does not have a published revision.",
                    DiagnosticStage::Mutation,
                    None,
                    "Open or refresh the project before preparing a mutation.",
                )
            })
    }

    pub fn project_root(&self, handle: ProjectHandle) -> Result<PathBuf, ContextError> {
        self.projects
            .get(&handle)
            .map(|state| state.project_root.clone())
            .ok_or_else(|| {
                ContextError::new(
                    "authoring_context.handle_invalid",
                    "Project handle is not open in this process.",
                    DiagnosticStage::Open,
                    None,
                    "Open the project and use the returned handle.",
                )
            })
    }

    pub fn capture_mutation_before(
        &self,
        handle: ProjectHandle,
        paths: &[String],
    ) -> Result<Vec<crate::ProjectMutationBeforeState>, ContextError> {
        let state = self.projects.get(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Mutation,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        crate::mutation::capture_before_states(&state.project_root, paths)
    }

    pub fn commit_mutation(
        &mut self,
        handle: ProjectHandle,
        mutation: crate::ProjectMutation,
    ) -> Result<crate::ProjectMutationReceipt, ContextError> {
        self.commit_mutation_controlled(handle, mutation, |_| Ok(()))
    }

    #[doc(hidden)]
    pub fn commit_mutation_controlled(
        &mut self,
        handle: ProjectHandle,
        mutation: crate::ProjectMutation,
        control: impl FnMut(&crate::MutationCommitProgress) -> Result<(), String>,
    ) -> Result<crate::ProjectMutationReceipt, ContextError> {
        let state = self.projects.get_mut(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Mutation,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        let outcome = crate::mutation::commit_mutation(
            &state.project_root,
            &state.project_id,
            &state.root_binding,
            mutation,
            control,
        )?;
        state.current_revision = Some(outcome.receipt.after_revision.clone());
        state.current_inventory = Some(outcome.inventory);
        Ok(outcome.receipt)
    }

    pub fn rollback_mutation(
        &mut self,
        handle: ProjectHandle,
        receipt: &crate::ProjectMutationReceipt,
    ) -> Result<crate::ProjectMutationRollbackReceipt, ContextError> {
        let state = self.projects.get_mut(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Rollback,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        let outcome = crate::mutation::rollback_mutation(
            &state.project_root,
            &state.project_id,
            &state.root_binding,
            receipt,
        )?;
        state.current_revision = Some(outcome.receipt.restored_revision.clone());
        state.current_inventory = Some(outcome.inventory);
        Ok(outcome.receipt)
    }

    pub fn mutation_receipt_for_journal(
        &self,
        handle: ProjectHandle,
        journal_relative_path: &str,
    ) -> Result<crate::ProjectMutationReceipt, ContextError> {
        let state = self.projects.get(&handle).ok_or_else(|| {
            ContextError::new(
                "authoring_context.handle_invalid",
                "Project handle is not open in this process.",
                DiagnosticStage::Rollback,
                None,
                "Open the project and use the returned handle.",
            )
        })?;
        crate::mutation::receipt_for_journal(&state.project_root, journal_relative_path)
    }
}

pub(crate) fn revision_for_inventory(
    project_id: &str,
    inventory: &CanonicalSourceInventory,
) -> (ProjectRevision, Vec<ProjectDiagnostic>) {
    let diagnostics = validate_current_manifest(inventory, project_id);
    let qualification = if diagnostics.is_empty() {
        ProjectQualification::Ready
    } else {
        ProjectQualification::Invalid
    };
    let diagnostics_digest = diagnostics_digest(&diagnostics);
    let revision = ProjectRevision {
        schema_version: PROJECT_REVISION_SCHEMA_VERSION.to_string(),
        portable_project_identity: project_id.to_string(),
        source_policy_version: SOURCE_POLICY_VERSION.to_string(),
        source_digest: inventory.source_digest().to_string(),
        revision_id: revision_id(project_id, inventory.source_digest()),
        qualification,
        diagnostics_digest,
    };
    (revision, diagnostics)
}

fn validate_current_manifest(
    inventory: &CanonicalSourceInventory,
    opened_project_id: &str,
) -> Vec<ProjectDiagnostic> {
    let manifest_path = inventory.project_root().join(PROJECT_MANIFEST_PATH);
    let Some(entry) = inventory.entry(PROJECT_MANIFEST_PATH) else {
        return vec![diagnostic(
            "authoring_context.manifest_unreadable",
            "Project manifest is missing from the canonical source inventory.",
            &manifest_path,
            "Restore project.aife.json and refresh again.",
        )];
    };
    let manifest_bytes = match inventory.read_verified(entry) {
        Ok(bytes) => bytes,
        Err(error) => return vec![error.diagnostic],
    };
    let manifest = match serde_json::from_slice::<ProjectManifestHeader>(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            return vec![diagnostic(
                "authoring_context.manifest_invalid",
                format!("Project manifest cannot be parsed: {error}"),
                &manifest_path,
                "Repair project.aife.json and refresh again.",
            )]
        }
    };
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        return vec![diagnostic(
            "authoring_context.manifest_schema_unsupported",
            format!(
                "Unsupported project manifest schema: {}",
                manifest.schema_version
            ),
            &manifest_path,
            format!("Migrate the manifest to {PROJECT_MANIFEST_SCHEMA_VERSION}."),
        )];
    }
    if let Err(error) = validate_project_id(
        &manifest.project_id,
        &manifest_path,
        DiagnosticStage::Refresh,
    ) {
        return vec![error.diagnostic];
    }
    if manifest.project_id != opened_project_id {
        return vec![diagnostic(
            "authoring_context.project_identity_changed",
            "The manifest projectId no longer matches the opened project binding.",
            &manifest_path,
            "Close this handle and reopen the project using its new identity.",
        )];
    }
    Vec::new()
}

fn validate_project_id(
    project_id: &str,
    manifest_path: &Path,
    stage: DiagnosticStage,
) -> Result<(), ContextError> {
    let valid = !project_id.is_empty()
        && project_id.len() <= 128
        && project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(ContextError::new(
            "authoring_context.project_id_invalid",
            "Project id must contain 1-128 ASCII letters, digits, '.', '-' or '_'.",
            stage,
            Some(manifest_path.display().to_string()),
            "Repair projectId in project.aife.json and retry.",
        ))
    }
}

fn revision_id(project_id: &str, source_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_REVISION_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(project_id.as_bytes());
    hasher.update([0]);
    hasher.update(SOURCE_POLICY_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(source_digest.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn diagnostics_digest(diagnostics: &[ProjectDiagnostic]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_DIAGNOSTICS_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    for diagnostic in diagnostics {
        let bytes = serde_json::to_vec(diagnostic)
            .expect("ProjectDiagnostic serialization is infallible for owned strings");
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    path: &Path,
    next_action: impl Into<String>,
) -> ProjectDiagnostic {
    ProjectDiagnostic {
        code: code.into(),
        message: message.into(),
        stage: DiagnosticStage::Refresh,
        path: Some(path.display().to_string()),
        next_action: next_action.into(),
    }
}

pub(crate) fn root_binding(project_root: &Path, project_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"opened-project-binding.v1");
    hasher.update([0]);
    hasher.update(project_id.as_bytes());
    hasher.update([0]);
    hasher.update(project_root.to_string_lossy().as_bytes());
    hasher.finalize().into()
}

pub(crate) fn reject_link_or_reparse(
    path: &Path,
    stage: DiagnosticStage,
) -> Result<(), ContextError> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if is_link_or_reparse(&metadata) {
        return Err(ContextError::new(
            "authoring_context.source_link_rejected",
            "Project authority does not follow symbolic links, junctions or reparse points.",
            stage,
            Some(path.display().to_string()),
            "Replace the link with a project-owned regular file or directory.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
