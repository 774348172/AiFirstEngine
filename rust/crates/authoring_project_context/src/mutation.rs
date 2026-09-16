use crate::authority_lock::ProjectAuthorityLock;
use crate::{CanonicalSourceInventory, ContextError, DiagnosticStage, ProjectRevision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const PROJECT_MUTATION_SCHEMA_VERSION: &str = "authoring-project-mutation.v1";
const MAX_MUTATION_OPERATIONS: usize = 128;
const MAX_MUTATION_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_MUTATION_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const JOURNAL_SCHEMA_VERSION: &str = "authoring-mutation-journal.v1";
pub const PROJECT_MUTATION_RECEIPT_SCHEMA_VERSION: &str = "authoring-mutation-receipt.v1";
pub const PROJECT_MUTATION_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str =
    "authoring-mutation-rollback-receipt.v1";
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMutationBeforeState {
    pub path: String,
    pub content_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectMutationOperation {
    CreateOrReplace { path: String, bytes: Vec<u8> },
    Delete { path: String },
    Move { from: String, to: String },
}

impl ProjectMutationOperation {
    fn paths(&self) -> [&str; 2] {
        match self {
            Self::CreateOrReplace { path, .. } | Self::Delete { path } => [path, ""],
            Self::Move { from, to } => [from, to],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMutation {
    pub schema_version: String,
    pub mutation_id: String,
    pub domain: String,
    pub expected_revision_id: String,
    pub validation_digest: String,
    pub declared_read_set: Vec<String>,
    pub declared_write_set: Vec<String>,
    pub expected_before: Vec<ProjectMutationBeforeState>,
    pub operations: Vec<ProjectMutationOperation>,
}

impl ProjectMutation {
    pub fn validate(&self) -> Result<(), ContextError> {
        if self.schema_version != PROJECT_MUTATION_SCHEMA_VERSION {
            return Err(mutation_error(
                "authoring_context.mutation_schema_unsupported",
                "ProjectMutation schema is unsupported.",
                None,
                format!("Regenerate the mutation using {PROJECT_MUTATION_SCHEMA_VERSION}."),
            ));
        }
        validate_token(&self.mutation_id, "mutation id")?;
        validate_token(&self.domain, "mutation domain")?;
        validate_digest(&self.expected_revision_id, "expected revision id")?;
        validate_digest(&self.validation_digest, "validation digest")?;
        if self.operations.is_empty() || self.operations.len() > MAX_MUTATION_OPERATIONS {
            return Err(mutation_error(
                "authoring_context.mutation_operation_count_invalid",
                format!("ProjectMutation must contain 1-{MAX_MUTATION_OPERATIONS} operations."),
                None,
                "Split or regenerate the mutation within the operation limit.",
            ));
        }

        let read_set = validate_path_set(&self.declared_read_set, "read")?;
        let write_set = validate_path_set(&self.declared_write_set, "write")?;
        if write_set.is_empty() {
            return Err(mutation_error(
                "authoring_context.mutation_write_set_empty",
                "ProjectMutation declared write set is empty.",
                None,
                "Declare every path changed by the mutation.",
            ));
        }
        if read_set.iter().any(|path| is_control_path(path))
            || write_set.iter().any(|path| is_control_path(path))
        {
            return Err(mutation_error(
                "authoring_context.mutation_control_path_rejected",
                "ProjectMutation cannot read or write AuthoringProjectContext control state.",
                None,
                "Remove .aife authority, transaction, receipt, or candidate paths.",
            ));
        }

        let mut before_paths = BTreeSet::new();
        for before in &self.expected_before {
            let path = validate_relative_path(&before.path)?;
            if is_control_path(&path) {
                return Err(mutation_error(
                    "authoring_context.mutation_control_path_rejected",
                    "ProjectMutation before state cannot bind authority control state.",
                    Some(path),
                    "Remove .aife authority, transaction, receipt, or candidate paths.",
                ));
            }
            if let Some(digest) = &before.content_digest {
                validate_digest(digest, "before content digest")?;
            }
            if !before_paths.insert(path.clone()) {
                return Err(mutation_error(
                    "authoring_context.mutation_before_duplicate",
                    "ProjectMutation contains duplicate before-state paths.",
                    Some(path),
                    "Bind each declared write path exactly once.",
                ));
            }
        }
        if before_paths != write_set {
            return Err(mutation_error(
                "authoring_context.mutation_before_set_mismatch",
                "ProjectMutation before states do not exactly match its declared write set.",
                None,
                "Provide one expected before hash or explicit missing state for every write path.",
            ));
        }

        let mut operation_paths = BTreeSet::new();
        let mut total_bytes = 0usize;
        for operation in &self.operations {
            if let ProjectMutationOperation::CreateOrReplace { bytes, .. } = operation {
                if bytes.len() > MAX_MUTATION_FILE_BYTES {
                    return Err(mutation_error(
                        "authoring_context.mutation_file_too_large",
                        format!("Mutation file exceeds {MAX_MUTATION_FILE_BYTES} bytes."),
                        None,
                        "Split the mutation or use the bounded asset import path.",
                    ));
                }
                total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
                    mutation_error(
                        "authoring_context.mutation_total_bytes_invalid",
                        "Mutation byte length overflowed the supported range.",
                        None,
                        "Split the mutation into bounded operations.",
                    )
                })?;
            }
            for raw_path in operation
                .paths()
                .into_iter()
                .filter(|path| !path.is_empty())
            {
                let path = validate_relative_path(raw_path)?;
                if is_control_path(&path) {
                    return Err(mutation_error(
                        "authoring_context.mutation_control_path_rejected",
                        "ProjectMutation cannot target AuthoringProjectContext control state.",
                        Some(path),
                        "Remove .aife authority, transaction, receipt, or candidate paths.",
                    ));
                }
                if !write_set.contains(&path) {
                    return Err(mutation_error(
                        "authoring_context.mutation_write_undeclared",
                        "Mutation operation targets a path outside its declared write set.",
                        Some(path),
                        "Declare the path and bind its expected before state.",
                    ));
                }
                if !operation_paths.insert(path.clone()) {
                    return Err(mutation_error(
                        "authoring_context.mutation_operation_path_duplicate",
                        "More than one mutation operation targets the same path.",
                        Some(path),
                        "Lower the desired result to one canonical operation per path.",
                    ));
                }
            }
        }
        if total_bytes > MAX_MUTATION_TOTAL_BYTES {
            return Err(mutation_error(
                "authoring_context.mutation_total_bytes_invalid",
                format!("Mutation bytes exceed {MAX_MUTATION_TOTAL_BYTES} total bytes."),
                None,
                "Split the mutation into bounded commits.",
            ));
        }
        if operation_paths != write_set {
            return Err(mutation_error(
                "authoring_context.mutation_write_set_unused",
                "Declared write set contains a path with no canonical operation.",
                None,
                "Remove unused paths or add the missing deterministic operation.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationCommitProgress {
    AfterPrepared,
    BeforeApply,
    AfterOperation { index: usize, path: String },
    BeforeVerification,
    AfterJournalCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRecoveryDisposition {
    NoRecovery,
    Aborted,
    Committed,
    RestoredBefore,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRecoveryTransactionReport {
    pub journal_path: String,
    pub mutation_id: String,
    pub prior_state: String,
    pub disposition: ProjectRecoveryDisposition,
    pub diagnostics: Vec<crate::ProjectDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRecoveryReport {
    pub disposition: ProjectRecoveryDisposition,
    pub transactions: Vec<ProjectRecoveryTransactionReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRecoveryProgress {
    AfterRestorePath { journal_path: String, path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMutationReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub mutation_id: String,
    pub domain: String,
    pub project_identity: String,
    pub root_binding_digest: String,
    pub validation_digest: String,
    pub before_revision: ProjectRevision,
    pub after_revision: ProjectRevision,
    pub declared_write_set: Vec<String>,
    pub actual_changed_paths: Vec<String>,
    pub changed_paths: Vec<String>,
    pub journal_path: String,
    pub receipt_path: String,
    pub binding_digest: String,
    pub rollback_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMutationRollbackReceipt {
    pub schema_version: String,
    pub mutation_id: String,
    pub source_receipt_id: String,
    pub restored_revision: ProjectRevision,
    pub replaced_revision: ProjectRevision,
    pub changed_paths: Vec<String>,
    pub rollback_receipt_path: String,
    pub binding_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalState {
    Prepared,
    Applying,
    Committed,
    RolledBack,
    Aborted,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JournalSnapshot {
    path: String,
    before_digest: Option<String>,
    after_digest: Option<String>,
    before_material_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MutationJournal {
    schema_version: String,
    mutation: ProjectMutation,
    state: JournalState,
    project_identity: String,
    root_binding_digest: String,
    before_revision: ProjectRevision,
    after_revision: Option<ProjectRevision>,
    snapshots: Vec<JournalSnapshot>,
    binding_digest: String,
}

pub(crate) struct CommitOutcome {
    pub receipt: ProjectMutationReceipt,
    pub inventory: CanonicalSourceInventory,
}

pub(crate) struct RollbackOutcome {
    pub receipt: ProjectMutationRollbackReceipt,
    pub inventory: CanonicalSourceInventory,
}

pub(crate) fn recover_project(project_root: &Path) -> Result<ProjectRecoveryReport, ContextError> {
    recover_project_controlled(project_root, |_| {})
}

#[doc(hidden)]
pub(crate) fn recover_project_controlled(
    project_root: &Path,
    mut control: impl FnMut(&ProjectRecoveryProgress),
) -> Result<ProjectRecoveryReport, ContextError> {
    let _authority = ProjectAuthorityLock::acquire(project_root, "open-recovery")
        .map_err(recovery_from_error)?;
    let journal_paths = recovery_journal_paths(project_root)?;
    let mut journals = Vec::with_capacity(journal_paths.len());
    for path in journal_paths {
        let relative = canonical_control_relative(project_root, &path)?;
        let journal: MutationJournal = read_json(&path)
            .map_err(|error| recovery_wrap("authoring_context.rollback_journal_invalid", error))?;
        validate_recovery_journal(project_root, &relative, &journal)?;
        journals.push((path, relative, journal));
    }

    reject_conflicting_recovery_journals(&journals)?;
    if let Some((path, _, _)) = journals
        .iter()
        .find(|(_, _, journal)| journal.state == JournalState::RecoveryRequired)
    {
        return Err(recovery_error(
            "authoring_context.recovery_required",
            "A mutation journal is already marked recovery_required.",
            Some(path.display().to_string()),
            "Preserve the transaction and resolve the unknown touched-path bytes.",
        ));
    }
    for (_, relative, journal) in &journals {
        if journal.state == JournalState::Committed
            && committed_receipt_path(project_root, journal).exists()
        {
            ensure_committed_receipt(project_root, relative, journal)?;
        }
    }
    let mut transactions = Vec::new();
    for (path, relative, mut journal) in journals {
        match journal.state {
            JournalState::Prepared | JournalState::Applying => {
                transactions.push(recover_incomplete_journal(
                    project_root,
                    &path,
                    &relative,
                    &mut journal,
                    &mut control,
                )?)
            }
            JournalState::Committed => {
                if ensure_committed_receipt(project_root, &relative, &journal)? {
                    transactions.push(recovery_transaction_report(
                        relative,
                        &journal,
                        JournalState::Committed,
                        ProjectRecoveryDisposition::Committed,
                    ));
                }
            }
            JournalState::RecoveryRequired => {
                return Err(recovery_error(
                    "authoring_context.recovery_required",
                    "A mutation journal is already marked recovery_required.",
                    Some(path.display().to_string()),
                    "Preserve the transaction and resolve the unknown touched-path bytes.",
                ));
            }
            JournalState::RolledBack | JournalState::Aborted => {}
        }
    }
    transactions.sort_by(|left, right| left.journal_path.cmp(&right.journal_path));
    let disposition = transactions.iter().fold(
        ProjectRecoveryDisposition::NoRecovery,
        |current, transaction| stronger_recovery_disposition(current, transaction.disposition),
    );
    Ok(ProjectRecoveryReport {
        disposition,
        transactions,
    })
}

fn recovery_journal_paths(project_root: &Path) -> Result<Vec<PathBuf>, ContextError> {
    let root = project_root.join(".aife/authoring/transactions");
    if !root.exists() {
        return Ok(Vec::new());
    }
    crate::context::reject_link_or_reparse(&root, DiagnosticStage::Recovery)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(&root).map_err(|error| {
        recovery_io_error(
            "authoring_context.recovery_journal_read_failed",
            error,
            &root,
        )
    })? {
        let entry = entry.map_err(|error| {
            recovery_io_error(
                "authoring_context.recovery_journal_read_failed",
                error,
                &root,
            )
        })?;
        crate::context::reject_link_or_reparse(&entry.path(), DiagnosticStage::Recovery)?;
        if !entry
            .file_type()
            .map_err(|error| {
                recovery_io_error(
                    "authoring_context.recovery_journal_read_failed",
                    error,
                    &entry.path(),
                )
            })?
            .is_dir()
        {
            continue;
        }
        let path = entry.path().join("journal.json");
        if path.is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn canonical_control_relative(project_root: &Path, path: &Path) -> Result<String, ContextError> {
    path.strip_prefix(project_root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .filter(|relative| {
            relative.starts_with(".aife/authoring/transactions/")
                && relative.ends_with("/journal.json")
        })
        .ok_or_else(|| {
            recovery_error(
                "authoring_context.recovery_journal_invalid",
                "Mutation journal escaped the project authority transaction root.",
                Some(path.display().to_string()),
                "Preserve the transaction and repair its canonical authority path.",
            )
        })
}

fn validate_recovery_journal(
    project_root: &Path,
    journal_relative: &str,
    journal: &MutationJournal,
) -> Result<(), ContextError> {
    let transaction_relative = journal_relative
        .strip_suffix("/journal.json")
        .expect("canonical journal path has suffix");
    let transaction_id = transaction_relative
        .rsplit('/')
        .next()
        .expect("canonical journal path has transaction id");
    let expected_root_binding = format!(
        "sha256:{}",
        hex_bytes(&crate::context::root_binding(
            project_root,
            &journal.project_identity
        ))
    );
    let actual_binding = journal_digest(journal).map_err(recovery_from_error)?;
    let invalid_field = if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        Some("schema_version")
    } else if journal.mutation.mutation_id != transaction_id {
        Some("transaction_id")
    } else if journal.root_binding_digest != expected_root_binding {
        Some("root_binding_digest")
    } else if journal.before_revision.portable_project_identity != journal.project_identity {
        Some("project_identity")
    } else if journal.mutation.expected_revision_id != journal.before_revision.revision_id {
        Some("before_revision")
    } else if actual_binding != journal.binding_digest {
        Some("binding_digest")
    } else {
        None
    };
    if let Some(invalid_field) = invalid_field {
        return Err(recovery_error(
            "authoring_context.recovery_journal_invalid",
            format!("Mutation journal field {invalid_field} is invalid."),
            Some(journal_relative.to_string()),
            "Preserve the transaction and reject automatic recovery.",
        ));
    }
    journal.mutation.validate().map_err(recovery_from_error)?;
    let revision_state_valid = match journal.state {
        JournalState::Committed => journal.after_revision.is_some(),
        JournalState::Prepared
        | JournalState::Applying
        | JournalState::Aborted
        | JournalState::RecoveryRequired => journal.after_revision.is_none(),
        JournalState::RolledBack => true,
    };
    if !revision_state_valid {
        return Err(recovery_error(
            "authoring_context.recovery_journal_invalid",
            "Mutation journal revision state is inconsistent.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and reject automatic recovery.",
        ));
    }

    let before = load_before_material(project_root, journal).map_err(recovery_from_error)?;
    let after = expected_after_states(&journal.mutation, &before).map_err(recovery_from_error)?;
    if before.len() != journal.mutation.declared_write_set.len()
        || journal.snapshots.len() != before.len()
    {
        return Err(recovery_error(
            "authoring_context.recovery_material_invalid",
            "Mutation recovery snapshot set does not match the declared write set.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and reject automatic recovery.",
        ));
    }
    let expected_before = journal
        .mutation
        .expected_before
        .iter()
        .map(|state| (state.path.as_str(), state.content_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    for (index, snapshot) in journal.snapshots.iter().enumerate() {
        let expected_material = snapshot
            .before_digest
            .as_ref()
            .map(|_| format!("{transaction_relative}/before/{index:04}.bin"));
        let actual_before_digest = before
            .get(&snapshot.path)
            .and_then(|bytes| bytes.as_deref())
            .map(content_digest);
        let actual_after_digest = after
            .get(&snapshot.path)
            .and_then(|bytes| bytes.as_deref())
            .map(content_digest);
        if expected_before.get(snapshot.path.as_str()) != Some(&snapshot.before_digest)
            || snapshot.before_digest != actual_before_digest
            || snapshot.after_digest != actual_after_digest
            || snapshot.before_material_path != expected_material
        {
            return Err(recovery_error(
                "authoring_context.recovery_material_invalid",
                "Mutation recovery material does not match the sealed before/after contract.",
                Some(snapshot.path.clone()),
                "Preserve the transaction and reject automatic recovery.",
            ));
        }
    }
    Ok(())
}

fn reject_conflicting_recovery_journals(
    journals: &[(PathBuf, String, MutationJournal)],
) -> Result<(), ContextError> {
    let active = journals
        .iter()
        .filter(|(_, _, journal)| {
            matches!(
                journal.state,
                JournalState::Prepared | JournalState::Applying
            )
        })
        .collect::<Vec<_>>();
    for (index, left) in active.iter().enumerate() {
        let left_paths = left
            .2
            .mutation
            .declared_write_set
            .iter()
            .collect::<BTreeSet<_>>();
        for right in active.iter().skip(index + 1) {
            if right
                .2
                .mutation
                .declared_write_set
                .iter()
                .any(|path| left_paths.contains(path))
            {
                return Err(recovery_error(
                    "authoring_context.recovery_journals_conflict",
                    "Multiple incomplete mutation journals claim the same project path.",
                    Some(format!("{}, {}", left.1, right.1)),
                    "Preserve both transactions and resolve the conflict explicitly.",
                ));
            }
        }
    }
    Ok(())
}

fn recover_incomplete_journal(
    project_root: &Path,
    journal_path: &Path,
    journal_relative: &str,
    journal: &mut MutationJournal,
    control: &mut impl FnMut(&ProjectRecoveryProgress),
) -> Result<ProjectRecoveryTransactionReport, ContextError> {
    let prior_state = journal.state;
    let before = load_before_material(project_root, journal).map_err(recovery_from_error)?;
    let after = expected_after_states(&journal.mutation, &before).map_err(recovery_from_error)?;
    let mut before_count = 0usize;
    let mut after_count = 0usize;
    let mut exact_after = BTreeMap::new();
    for (path, before_bytes) in &before {
        let current = read_optional_regular(project_root, path).map_err(recovery_from_error)?;
        let after_bytes = after.get(path).expect("validated recovery path");
        if current == *before_bytes {
            before_count += 1;
        } else if current == *after_bytes {
            after_count += 1;
            exact_after.insert(path.clone(), before_bytes.clone());
        } else {
            mark_recovery_required(journal, journal_path).map_err(recovery_from_error)?;
            return Err(recovery_error(
                "authoring_context.recovery_required",
                "A touched path contains bytes that are neither the sealed before nor after state.",
                Some(path.clone()),
                "Preserve the unknown bytes and resolve the transaction explicitly.",
            ));
        }
    }

    if after_count == 0 {
        journal.state = JournalState::Aborted;
        journal.binding_digest = journal_digest(journal).map_err(recovery_from_error)?;
        write_json_atomic(journal_path, journal).map_err(recovery_from_error)?;
        return Ok(recovery_transaction_report(
            journal_relative.to_string(),
            journal,
            prior_state,
            ProjectRecoveryDisposition::Aborted,
        ));
    }

    restore_recovery_states(project_root, journal_relative, &exact_after, control).map_err(
        |error| {
            let _ = mark_recovery_required(journal, journal_path);
            recovery_from_error(error)
        },
    )?;
    if first_state_mismatch(project_root, &before)
        .map_err(recovery_from_error)?
        .is_some()
    {
        mark_recovery_required(journal, journal_path).map_err(recovery_from_error)?;
        return Err(recovery_error(
            "authoring_context.recovery_required",
            "Recovery could not establish the sealed before state for every touched path.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and inspect concurrent external writes.",
        ));
    }

    if before_count == 0 {
        let restored_inventory =
            CanonicalSourceInventory::capture(project_root).map_err(recovery_from_error)?;
        let (restored_revision, restored_diagnostics) =
            crate::context::revision_for_inventory(&journal.project_identity, &restored_inventory);
        if restored_diagnostics.is_empty() && restored_revision == journal.before_revision {
            restore_states(project_root, &after).map_err(|error| {
                let _ = mark_recovery_required(journal, journal_path);
                recovery_from_error(error)
            })?;
            let after_inventory =
                CanonicalSourceInventory::capture(project_root).map_err(recovery_from_error)?;
            let (after_revision, after_diagnostics) =
                crate::context::revision_for_inventory(&journal.project_identity, &after_inventory);
            let actual_source_changes = inventory_changes(&restored_inventory, &after_inventory);
            let allowed_source_changes = journal
                .mutation
                .declared_write_set
                .iter()
                .filter(|path| {
                    crate::CanonicalSourcePolicy::allows_change(project_root, Path::new(path))
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            let unknown_source_changes = actual_source_changes
                .difference(&allowed_source_changes)
                .next()
                .is_some();
            let touched_paths_match = first_state_mismatch(project_root, &after)
                .map_err(recovery_from_error)?
                .is_none();
            if after_diagnostics.is_empty() && !unknown_source_changes && touched_paths_match {
                journal.after_revision = Some(after_revision);
                journal.state = JournalState::Committed;
                journal.binding_digest = journal_digest(journal).map_err(recovery_from_error)?;
                write_json_atomic(journal_path, journal).map_err(recovery_from_error)?;
                ensure_committed_receipt(project_root, journal_relative, journal)?;
                return Ok(recovery_transaction_report(
                    journal_relative.to_string(),
                    journal,
                    prior_state,
                    ProjectRecoveryDisposition::Committed,
                ));
            }
            let current =
                capture_snapshot_bytes(project_root, &journal.mutation.declared_write_set)
                    .map_err(recovery_from_error)?;
            let mut exact_after = BTreeMap::new();
            for (path, bytes) in &current {
                if after.get(path) == Some(bytes) {
                    exact_after.insert(
                        path.clone(),
                        before.get(path).cloned().expect("validated recovery path"),
                    );
                } else if before.get(path) != Some(bytes) {
                    mark_recovery_required(journal, journal_path).map_err(recovery_from_error)?;
                    return Err(recovery_error(
                        "authoring_context.recovery_required",
                        "A touched path changed to unknown bytes while recovery was finalizing.",
                        Some(path.clone()),
                        "Preserve the unknown bytes and resolve the transaction explicitly.",
                    ));
                }
            }
            restore_states(project_root, &exact_after).map_err(recovery_from_error)?;
        }
    }

    journal.state = JournalState::RolledBack;
    journal.binding_digest = journal_digest(journal).map_err(recovery_from_error)?;
    write_json_atomic(journal_path, journal).map_err(recovery_from_error)?;
    Ok(recovery_transaction_report(
        journal_relative.to_string(),
        journal,
        prior_state,
        ProjectRecoveryDisposition::RestoredBefore,
    ))
}

fn restore_recovery_states(
    project_root: &Path,
    journal_relative: &str,
    states: &BTreeMap<String, Option<Vec<u8>>>,
    control: &mut impl FnMut(&ProjectRecoveryProgress),
) -> Result<(), ContextError> {
    for (path, bytes) in states.iter().rev() {
        if let Some(bytes) = bytes {
            write_project_file_atomic(project_root, path, bytes)?;
        } else {
            remove_project_file(project_root, path)?;
        }
        control(&ProjectRecoveryProgress::AfterRestorePath {
            journal_path: journal_relative.to_string(),
            path: path.clone(),
        });
    }
    Ok(())
}

fn ensure_committed_receipt(
    project_root: &Path,
    journal_relative: &str,
    journal: &MutationJournal,
) -> Result<bool, ContextError> {
    let after_revision = journal.after_revision.as_ref().ok_or_else(|| {
        recovery_error(
            "authoring_context.recovery_journal_invalid",
            "Committed mutation journal has no after revision.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and reject automatic recovery.",
        )
    })?;
    let receipt_id = committed_receipt_id(journal);
    let receipt_relative = format!(".aife/authoring/receipts/{receipt_id}.json");
    let receipt_path = project_root.join(&receipt_relative);
    if receipt_path.exists() {
        let receipt: ProjectMutationReceipt = read_json(&receipt_path)
            .map_err(|error| recovery_wrap("authoring_context.rollback_receipt_invalid", error))?;
        validate_mutation_receipt(&receipt).map_err(recovery_from_error)?;
        if receipt.receipt_id != receipt_id
            || receipt.mutation_id != journal.mutation.mutation_id
            || receipt.project_identity != journal.project_identity
            || receipt.root_binding_digest != journal.root_binding_digest
            || receipt.before_revision != journal.before_revision
            || receipt.after_revision != *after_revision
            || receipt.journal_path != journal_relative
            || receipt.receipt_path != receipt_relative
            || receipt.validation_digest != journal.mutation.validation_digest
            || receipt.declared_write_set != journal.mutation.declared_write_set
        {
            return Err(recovery_error(
                "authoring_context.recovery_receipt_invalid",
                "Committed mutation receipt does not match its sealed journal.",
                Some(receipt_relative),
                "Preserve the receipt and journal and reject automatic recovery.",
            ));
        }
        return Ok(false);
    }

    let before = load_before_material(project_root, journal).map_err(recovery_from_error)?;
    let after = expected_after_states(&journal.mutation, &before).map_err(recovery_from_error)?;
    if first_state_mismatch(project_root, &after)
        .map_err(recovery_from_error)?
        .is_some()
    {
        return Err(recovery_error(
            "authoring_context.recovery_receipt_unprovable",
            "Committed journal is missing its receipt and current touched paths do not match after state.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and restore the exact committed revision before recovery.",
        ));
    }
    let inventory = CanonicalSourceInventory::capture(project_root).map_err(recovery_from_error)?;
    let (revision, diagnostics) =
        crate::context::revision_for_inventory(&journal.project_identity, &inventory);
    if !diagnostics.is_empty() || revision != *after_revision {
        return Err(recovery_error(
            "authoring_context.recovery_receipt_unprovable",
            "Committed journal is missing its receipt and the exact after revision is not current.",
            Some(journal_relative.to_string()),
            "Preserve the transaction and restore the exact committed revision before recovery.",
        ));
    }
    let changed_paths = before
        .iter()
        .filter(|(path, bytes)| after.get(*path) != Some(*bytes))
        .filter(|(path, _)| {
            crate::CanonicalSourcePolicy::allows_change(project_root, Path::new(path.as_str()))
        })
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let mut receipt = ProjectMutationReceipt {
        schema_version: PROJECT_MUTATION_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id,
        mutation_id: journal.mutation.mutation_id.clone(),
        domain: journal.mutation.domain.clone(),
        project_identity: journal.project_identity.clone(),
        root_binding_digest: journal.root_binding_digest.clone(),
        validation_digest: journal.mutation.validation_digest.clone(),
        before_revision: journal.before_revision.clone(),
        after_revision: after_revision.clone(),
        declared_write_set: journal.mutation.declared_write_set.clone(),
        actual_changed_paths: changed_paths.clone(),
        changed_paths,
        journal_path: journal_relative.to_string(),
        receipt_path: receipt_relative,
        binding_digest: String::new(),
        rollback_available: true,
    };
    receipt.binding_digest = receipt_digest(&receipt).map_err(recovery_from_error)?;
    write_json_atomic(&receipt_path, &receipt).map_err(recovery_from_error)?;
    Ok(true)
}

fn committed_receipt_id(journal: &MutationJournal) -> String {
    format!(
        "receipt-{}",
        &content_digest(journal.binding_digest.as_bytes())[7..]
    )
}

fn committed_receipt_path(project_root: &Path, journal: &MutationJournal) -> PathBuf {
    project_root
        .join(".aife/authoring/receipts")
        .join(format!("{}.json", committed_receipt_id(journal)))
}

fn recovery_transaction_report(
    journal_path: String,
    journal: &MutationJournal,
    prior_state: JournalState,
    disposition: ProjectRecoveryDisposition,
) -> ProjectRecoveryTransactionReport {
    ProjectRecoveryTransactionReport {
        journal_path,
        mutation_id: journal.mutation.mutation_id.clone(),
        prior_state: journal_state_name(prior_state).to_string(),
        disposition,
        diagnostics: Vec::new(),
    }
}

fn journal_state_name(state: JournalState) -> &'static str {
    match state {
        JournalState::Prepared => "prepared",
        JournalState::Applying => "applying",
        JournalState::Committed => "committed",
        JournalState::RolledBack => "rolled_back",
        JournalState::Aborted => "aborted",
        JournalState::RecoveryRequired => "recovery_required",
    }
}

fn stronger_recovery_disposition(
    left: ProjectRecoveryDisposition,
    right: ProjectRecoveryDisposition,
) -> ProjectRecoveryDisposition {
    fn rank(value: ProjectRecoveryDisposition) -> u8 {
        match value {
            ProjectRecoveryDisposition::NoRecovery => 0,
            ProjectRecoveryDisposition::Aborted => 1,
            ProjectRecoveryDisposition::Committed => 2,
            ProjectRecoveryDisposition::RestoredBefore => 3,
            ProjectRecoveryDisposition::RecoveryRequired => 4,
        }
    }
    if rank(right) > rank(left) {
        right
    } else {
        left
    }
}

fn recovery_from_error(error: ContextError) -> ContextError {
    ContextError::new(
        error.diagnostic.code,
        error.diagnostic.message,
        DiagnosticStage::Recovery,
        error.diagnostic.path,
        error.diagnostic.next_action,
    )
}

fn recovery_wrap(code: &str, error: ContextError) -> ContextError {
    ContextError::new(
        code,
        error.diagnostic.message,
        DiagnosticStage::Recovery,
        error.diagnostic.path,
        error.diagnostic.next_action,
    )
}

fn recovery_io_error(code: &str, error: std::io::Error, path: &Path) -> ContextError {
    recovery_error(
        code,
        error.to_string(),
        Some(path.display().to_string()),
        "Preserve transaction evidence and inspect filesystem access.",
    )
}

fn recovery_error(
    code: &str,
    message: impl Into<String>,
    path: Option<String>,
    next_action: impl Into<String>,
) -> ContextError {
    ContextError::new(code, message, DiagnosticStage::Recovery, path, next_action)
}

#[cfg(test)]
pub(crate) fn clone_incomplete_journal_for_test(
    project_root: &Path,
    source_mutation_id: &str,
    target_mutation_id: &str,
) -> Result<(), ContextError> {
    validate_token(target_mutation_id, "mutation id")?;
    let source_relative = format!(".aife/authoring/transactions/{source_mutation_id}/journal.json");
    let source_path = project_root.join(&source_relative);
    let mut journal: MutationJournal = read_json(&source_path)?;
    if !matches!(
        journal.state,
        JournalState::Prepared | JournalState::Applying
    ) {
        return Err(mutation_error(
            "authoring_context.test_journal_not_incomplete",
            "Test journal clone requires an incomplete source journal.",
            Some(source_relative),
            "Use a prepared or applying test journal.",
        ));
    }
    let target_relative = format!(".aife/authoring/transactions/{target_mutation_id}");
    let target_root = project_root.join(&target_relative);
    fs::create_dir(&target_root).map_err(|error| {
        io_error(
            "authoring_context.mutation_journal_write_failed",
            error,
            &target_root,
        )
    })?;
    let target_before = target_root.join("before");
    fs::create_dir(&target_before).map_err(|error| {
        io_error(
            "authoring_context.mutation_journal_write_failed",
            error,
            &target_before,
        )
    })?;
    for (index, snapshot) in journal.snapshots.iter_mut().enumerate() {
        if let Some(source_material) = &snapshot.before_material_path {
            let bytes = fs::read(project_root.join(source_material)).map_err(|error| {
                io_error(
                    "authoring_context.mutation_journal_read_failed",
                    error,
                    &project_root.join(source_material),
                )
            })?;
            let target_material = format!("{target_relative}/before/{index:04}.bin");
            write_bytes_new(&project_root.join(&target_material), &bytes)?;
            snapshot.before_material_path = Some(target_material);
        }
    }
    journal.mutation.mutation_id = target_mutation_id.to_string();
    journal.binding_digest = journal_digest(&journal)?;
    write_json_atomic(&target_root.join("journal.json"), &journal)
}

pub(crate) fn capture_before_states(
    project_root: &Path,
    paths: &[String],
) -> Result<Vec<ProjectMutationBeforeState>, ContextError> {
    let mut result = Vec::with_capacity(paths.len());
    let mut seen = BTreeSet::new();
    for raw_path in paths {
        let path = validate_relative_path(raw_path)?;
        if !seen.insert(path.clone()) {
            return Err(mutation_error(
                "authoring_context.mutation_path_duplicate",
                "Cannot capture a mutation path more than once.",
                Some(path),
                "Request each canonical path once.",
            ));
        }
        let bytes = read_optional_regular(project_root, &path)?;
        result.push(ProjectMutationBeforeState {
            path,
            content_digest: bytes.as_deref().map(content_digest),
        });
    }
    Ok(result)
}

pub(crate) fn receipt_for_journal(
    project_root: &Path,
    journal_relative: &str,
) -> Result<ProjectMutationReceipt, ContextError> {
    let normalized = journal_relative.replace('\\', "/");
    if normalized != journal_relative
        || !normalized.starts_with(".aife/authoring/transactions/")
        || !normalized.ends_with("/journal.json")
        || Path::new(&normalized)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(rollback_error(
            "authoring_context.rollback_journal_path_invalid",
            "Mutation journal path is not a canonical authority transaction path.",
            Some(journal_relative.to_string()),
            "Use the exact journal path returned by mutation commit.",
        ));
    }
    let journal_path = project_root.join(&normalized);
    let journal: MutationJournal = read_json(&journal_path)
        .map_err(|error| rollback_wrap("authoring_context.rollback_journal_invalid", error))?;
    if journal.schema_version != JOURNAL_SCHEMA_VERSION
        || journal.state != JournalState::Committed
        || journal_digest(&journal).as_deref() != Ok(journal.binding_digest.as_str())
    {
        return Err(rollback_error(
            "authoring_context.rollback_journal_invalid",
            "Mutation journal is not a sealed committed transaction.",
            Some(journal_path.display().to_string()),
            "Preserve the journal and reject rollback.",
        ));
    }
    let receipt_id = format!(
        "receipt-{}",
        &content_digest(journal.binding_digest.as_bytes())[7..]
    );
    let receipt_path = project_root
        .join(".aife/authoring/receipts")
        .join(format!("{receipt_id}.json"));
    let receipt: ProjectMutationReceipt = read_json(&receipt_path)
        .map_err(|error| rollback_wrap("authoring_context.rollback_receipt_invalid", error))?;
    validate_mutation_receipt(&receipt)?;
    if receipt.receipt_id != receipt_id || receipt.journal_path != normalized {
        return Err(rollback_error(
            "authoring_context.rollback_receipt_invalid",
            "Mutation receipt does not bind the requested journal.",
            Some(receipt_path.display().to_string()),
            "Reject the receipt and preserve transaction evidence.",
        ));
    }
    Ok(receipt)
}

pub(crate) fn commit_mutation(
    project_root: &Path,
    project_identity: &str,
    root_binding: &[u8; 32],
    mutation: ProjectMutation,
    mut control: impl FnMut(&MutationCommitProgress) -> Result<(), String>,
) -> Result<CommitOutcome, ContextError> {
    mutation.validate()?;
    let _authority = ProjectAuthorityLock::acquire(project_root, &mutation.mutation_id)?;
    reject_nonterminal_journal(project_root)?;

    let before_inventory = CanonicalSourceInventory::capture(project_root)?;
    let (before_revision, diagnostics) =
        crate::context::revision_for_inventory(project_identity, &before_inventory);
    if !diagnostics.is_empty() {
        return Err(mutation_error(
            "authoring_context.mutation_project_unqualified",
            "Project is not qualified for mutation commit.",
            Some(project_root.display().to_string()),
            "Resolve current project diagnostics and prepare a new mutation.",
        ));
    }
    if mutation.expected_revision_id != before_revision.revision_id {
        return Err(mutation_error(
            "authoring_context.mutation_revision_drifted",
            format!(
                "Expected revision {} but current revision is {}.",
                mutation.expected_revision_id, before_revision.revision_id
            ),
            Some(project_root.display().to_string()),
            "Refresh, reread affected files, and prepare a new mutation.",
        ));
    }

    let before_by_path = capture_snapshot_bytes(project_root, &mutation.declared_write_set)?;
    verify_expected_before(&mutation, &before_by_path)?;
    let after_by_path = expected_after_states(&mutation, &before_by_path)?;
    let root_binding_digest = format!("sha256:{}", hex_bytes(root_binding));
    let transaction_relative = format!(".aife/authoring/transactions/{}", mutation.mutation_id);
    let transaction_root = project_root.join(&transaction_relative);
    fs::create_dir_all(transaction_root.parent().expect("transaction has parent")).map_err(
        |e| {
            io_error(
                "authoring_context.mutation_journal_write_failed",
                e,
                &transaction_root,
            )
        },
    )?;
    fs::create_dir(&transaction_root).map_err(|error| {
        mutation_error(
            "authoring_context.mutation_transaction_exists",
            format!("Mutation transaction root cannot be created: {error}"),
            Some(transaction_root.display().to_string()),
            "Use a new mutation id or resolve the existing transaction.",
        )
    })?;
    let snapshots = persist_before_material(
        project_root,
        &transaction_relative,
        &before_by_path,
        &after_by_path,
    )?;
    let journal_relative = format!("{transaction_relative}/journal.json");
    let journal_path = project_root.join(&journal_relative);
    let mut journal = MutationJournal {
        schema_version: JOURNAL_SCHEMA_VERSION.to_string(),
        mutation: mutation.clone(),
        state: JournalState::Prepared,
        project_identity: project_identity.to_string(),
        root_binding_digest: root_binding_digest.clone(),
        before_revision: before_revision.clone(),
        after_revision: None,
        snapshots,
        binding_digest: String::new(),
    };
    journal.binding_digest = journal_digest(&journal)?;
    write_json_atomic(&journal_path, &journal)?;
    if let Err(reason) = control(&MutationCommitProgress::AfterPrepared) {
        journal.state = JournalState::Aborted;
        journal.binding_digest = journal_digest(&journal)?;
        write_json_atomic(&journal_path, &journal)?;
        return Err(control_error(reason, project_root));
    }
    journal.state = JournalState::Applying;
    journal.binding_digest = journal_digest(&journal)?;
    write_json_atomic(&journal_path, &journal)?;

    let mut applied_state = before_by_path.clone();
    if let Err(reason) = control(&MutationCommitProgress::BeforeApply) {
        compensate_or_require_recovery(
            project_root,
            &before_by_path,
            &applied_state,
            &mut journal,
            &journal_path,
        )?;
        return Err(control_error(reason, project_root));
    }
    for (index, operation) in mutation.operations.iter().enumerate() {
        if let Err(error) = apply_operation(project_root, operation, &before_by_path) {
            compensate_or_require_recovery(
                project_root,
                &before_by_path,
                &applied_state,
                &mut journal,
                &journal_path,
            )?;
            return Err(error);
        }
        for path in operation
            .paths()
            .into_iter()
            .filter(|path| !path.is_empty())
        {
            applied_state.insert(
                path.to_string(),
                after_by_path.get(path).cloned().unwrap_or(None),
            );
        }
        let progress_path = match operation {
            ProjectMutationOperation::CreateOrReplace { path, .. }
            | ProjectMutationOperation::Delete { path } => path.clone(),
            ProjectMutationOperation::Move { to, .. } => to.clone(),
        };
        if let Err(reason) = control(&MutationCommitProgress::AfterOperation {
            index,
            path: progress_path,
        }) {
            compensate_or_require_recovery(
                project_root,
                &before_by_path,
                &applied_state,
                &mut journal,
                &journal_path,
            )?;
            return Err(control_error(reason, project_root));
        }
    }
    if let Err(reason) = control(&MutationCommitProgress::BeforeVerification) {
        compensate_or_require_recovery(
            project_root,
            &before_by_path,
            &applied_state,
            &mut journal,
            &journal_path,
        )?;
        return Err(control_error(reason, project_root));
    }

    if let Some(path) = first_state_mismatch(project_root, &after_by_path)? {
        journal.state = JournalState::RecoveryRequired;
        journal.binding_digest = journal_digest(&journal)?;
        write_json_atomic(&journal_path, &journal)?;
        return Err(mutation_error(
            "authoring_context.mutation_recovery_required",
            "A mutation write path contains unknown bytes after apply.",
            Some(path),
            "Preserve the transaction and perform explicit recovery; do not retry commit.",
        ));
    }

    let after_inventory = CanonicalSourceInventory::capture(project_root)?;
    let actual_source_changes = inventory_changes(&before_inventory, &after_inventory);
    let allowed_source_changes = mutation
        .declared_write_set
        .iter()
        .filter(|path| crate::CanonicalSourcePolicy::allows_change(project_root, Path::new(path)))
        .cloned()
        .collect::<BTreeSet<_>>();
    let unknown = actual_source_changes
        .difference(&allowed_source_changes)
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        compensate_or_require_recovery(
            project_root,
            &before_by_path,
            &after_by_path,
            &mut journal,
            &journal_path,
        )?;
        return Err(mutation_error(
            "authoring_context.mutation_unknown_external_write",
            format!(
                "Unknown canonical source changes occurred during commit: {}",
                unknown.join(", ")
            ),
            Some(project_root.display().to_string()),
            "Keep the external changes, refresh, and prepare a new mutation.",
        ));
    }

    let (after_revision, after_diagnostics) =
        crate::context::revision_for_inventory(project_identity, &after_inventory);
    if !after_diagnostics.is_empty() {
        compensate_or_require_recovery(
            project_root,
            &before_by_path,
            &after_by_path,
            &mut journal,
            &journal_path,
        )?;
        return Err(mutation_error(
            "authoring_context.mutation_after_unqualified",
            "Mutation produced an unqualified project revision.",
            Some(project_root.display().to_string()),
            "Inspect validation and prepare a corrected mutation.",
        ));
    }
    journal.after_revision = Some(after_revision.clone());
    journal.state = JournalState::Committed;
    journal.binding_digest = journal_digest(&journal)?;
    write_json_atomic(&journal_path, &journal)?;
    if let Err(reason) = control(&MutationCommitProgress::AfterJournalCommitted) {
        return Err(control_error(reason, project_root));
    }

    let receipt_id = format!(
        "receipt-{}",
        &content_digest(journal.binding_digest.as_bytes())[7..]
    );
    let receipt_relative = format!(".aife/authoring/receipts/{receipt_id}.json");
    let receipt_path = project_root.join(&receipt_relative);
    let mut receipt = ProjectMutationReceipt {
        schema_version: PROJECT_MUTATION_RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id,
        mutation_id: mutation.mutation_id,
        domain: mutation.domain,
        project_identity: project_identity.to_string(),
        root_binding_digest,
        validation_digest: mutation.validation_digest,
        before_revision,
        after_revision,
        declared_write_set: mutation.declared_write_set.clone(),
        actual_changed_paths: actual_source_changes.iter().cloned().collect(),
        changed_paths: actual_source_changes.into_iter().collect(),
        journal_path: journal_relative,
        receipt_path: receipt_relative,
        binding_digest: String::new(),
        rollback_available: true,
    };
    receipt.binding_digest = receipt_digest(&receipt)?;
    write_json_atomic(&receipt_path, &receipt)?;
    Ok(CommitOutcome {
        receipt,
        inventory: after_inventory,
    })
}

pub(crate) fn rollback_mutation(
    project_root: &Path,
    project_identity: &str,
    root_binding: &[u8; 32],
    receipt: &ProjectMutationReceipt,
) -> Result<RollbackOutcome, ContextError> {
    validate_mutation_receipt(receipt)?;
    let expected_root_binding = format!("sha256:{}", hex_bytes(root_binding));
    if receipt.project_identity != project_identity
        || receipt.root_binding_digest != expected_root_binding
    {
        return Err(rollback_error(
            "authoring_context.rollback_project_binding_mismatch",
            "Mutation receipt does not belong to this opened project root.",
            Some(project_root.display().to_string()),
            "Open the original project root and use its exact receipt.",
        ));
    }
    let _authority = ProjectAuthorityLock::acquire(project_root, &receipt.mutation_id)?;
    reject_nonterminal_journal(project_root)?;

    let persisted_receipt: ProjectMutationReceipt =
        read_json(&project_root.join(&receipt.receipt_path))
            .map_err(|error| rollback_wrap("authoring_context.rollback_receipt_invalid", error))?;
    if persisted_receipt != *receipt {
        return Err(rollback_error(
            "authoring_context.rollback_receipt_invalid",
            "Mutation receipt does not match its sealed persisted record.",
            Some(receipt.receipt_path.clone()),
            "Reject the receipt and preserve transaction evidence.",
        ));
    }
    let journal_path = project_root.join(&receipt.journal_path);
    let mut journal: MutationJournal = read_json(&journal_path)
        .map_err(|error| rollback_wrap("authoring_context.rollback_journal_invalid", error))?;
    if journal.schema_version != JOURNAL_SCHEMA_VERSION
        || journal.state != JournalState::Committed
        || journal.project_identity != project_identity
        || journal.root_binding_digest != expected_root_binding
        || journal.mutation.mutation_id != receipt.mutation_id
        || journal.after_revision.as_ref() != Some(&receipt.after_revision)
        || journal_digest(&journal).as_deref() != Ok(journal.binding_digest.as_str())
    {
        return Err(rollback_error(
            "authoring_context.rollback_journal_invalid",
            "Mutation journal binding or terminal state is invalid.",
            Some(journal_path.display().to_string()),
            "Preserve the transaction and perform explicit recovery.",
        ));
    }

    let current_inventory = CanonicalSourceInventory::capture(project_root)?;
    let (current_revision, diagnostics) =
        crate::context::revision_for_inventory(project_identity, &current_inventory);
    if !diagnostics.is_empty() || current_revision != receipt.after_revision {
        return Err(rollback_error(
            "authoring_context.rollback_revision_drifted",
            format!(
                "Rollback requires exact revision {} but current revision is {}.",
                receipt.after_revision.revision_id, current_revision.revision_id
            ),
            Some(project_root.display().to_string()),
            "Review intervening changes and use explicit merge or recovery.",
        ));
    }

    let before = load_before_material(project_root, &journal)?;
    let after = expected_after_states(&journal.mutation, &before)?;
    if let Some(path) = first_state_mismatch(project_root, &after)? {
        return Err(rollback_error(
            "authoring_context.rollback_write_path_drifted",
            "A mutation write path no longer matches the sealed applied state.",
            Some(path),
            "Review intervening derived or source writes before recovery.",
        ));
    }

    restore_states(project_root, &before).map_err(|error| {
        let _ = mark_recovery_required(&mut journal, &journal_path);
        rollback_wrap("authoring_context.rollback_recovery_required", error)
    })?;
    let restored_inventory = CanonicalSourceInventory::capture(project_root)?;
    let (restored_revision, restored_diagnostics) =
        crate::context::revision_for_inventory(project_identity, &restored_inventory);
    if !restored_diagnostics.is_empty() || restored_revision != receipt.before_revision {
        if first_state_mismatch(project_root, &before)?.is_none() {
            let _ = restore_states(project_root, &after);
        }
        return Err(rollback_error(
            "authoring_context.rollback_revision_mismatch",
            "Rollback did not restore the exact sealed before revision.",
            Some(project_root.display().to_string()),
            "Preserve transaction evidence and perform explicit recovery.",
        ));
    }

    journal.state = JournalState::RolledBack;
    journal.binding_digest = journal_digest(&journal)?;
    write_json_atomic(&journal_path, &journal)?;
    let rollback_relative = format!(
        ".aife/authoring/transactions/{}/rollback-receipt.json",
        receipt.mutation_id
    );
    let mut rollback_receipt = ProjectMutationRollbackReceipt {
        schema_version: PROJECT_MUTATION_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
        mutation_id: receipt.mutation_id.clone(),
        source_receipt_id: receipt.receipt_id.clone(),
        restored_revision,
        replaced_revision: receipt.after_revision.clone(),
        changed_paths: receipt.changed_paths.clone(),
        rollback_receipt_path: rollback_relative.clone(),
        binding_digest: String::new(),
    };
    rollback_receipt.binding_digest = rollback_receipt_digest(&rollback_receipt)?;
    write_json_atomic(&project_root.join(&rollback_relative), &rollback_receipt)?;
    Ok(RollbackOutcome {
        receipt: rollback_receipt,
        inventory: restored_inventory,
    })
}

fn validate_mutation_receipt(receipt: &ProjectMutationReceipt) -> Result<(), ContextError> {
    if receipt.schema_version != PROJECT_MUTATION_RECEIPT_SCHEMA_VERSION
        || !receipt.rollback_available
        || receipt_digest(receipt).as_deref() != Ok(receipt.binding_digest.as_str())
    {
        return Err(rollback_error(
            "authoring_context.rollback_receipt_invalid",
            "Mutation receipt schema or binding digest is invalid.",
            Some(receipt.receipt_path.clone()),
            "Reject the receipt and preserve transaction evidence.",
        ));
    }
    validate_digest(&receipt.binding_digest, "receipt binding digest")
        .map_err(|error| rollback_wrap("authoring_context.rollback_receipt_invalid", error))
}

fn load_before_material(
    project_root: &Path,
    journal: &MutationJournal,
) -> Result<BTreeMap<String, Option<Vec<u8>>>, ContextError> {
    let mut result = BTreeMap::new();
    for snapshot in &journal.snapshots {
        let bytes = match &snapshot.before_material_path {
            Some(relative) => {
                let path = project_root.join(relative);
                let bytes = fs::read(&path).map_err(|error| {
                    rollback_error(
                        "authoring_context.rollback_material_invalid",
                        format!("Rollback material cannot be read: {error}"),
                        Some(path.display().to_string()),
                        "Preserve the transaction and perform explicit recovery.",
                    )
                })?;
                if Some(content_digest(&bytes)) != snapshot.before_digest {
                    return Err(rollback_error(
                        "authoring_context.rollback_material_invalid",
                        "Rollback material digest does not match the sealed journal.",
                        Some(path.display().to_string()),
                        "Reject tampered rollback material.",
                    ));
                }
                Some(bytes)
            }
            None if snapshot.before_digest.is_none() => None,
            None => {
                return Err(rollback_error(
                    "authoring_context.rollback_material_invalid",
                    "Rollback material is missing for an existing before file.",
                    Some(snapshot.path.clone()),
                    "Preserve the transaction and perform explicit recovery.",
                ))
            }
        };
        result.insert(snapshot.path.clone(), bytes);
    }
    Ok(result)
}

fn mark_recovery_required(journal: &mut MutationJournal, path: &Path) -> Result<(), ContextError> {
    journal.state = JournalState::RecoveryRequired;
    journal.binding_digest = journal_digest(journal)?;
    write_json_atomic(path, journal)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ContextError> {
    let bytes = fs::read(path).map_err(|error| {
        io_error(
            "authoring_context.mutation_control_read_failed",
            error,
            path,
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        mutation_error(
            "authoring_context.mutation_control_decode_failed",
            format!("Mutation control record cannot be decoded: {error}"),
            Some(path.display().to_string()),
            "Preserve the record and perform explicit recovery.",
        )
    })
}

fn rollback_receipt_digest(
    receipt: &ProjectMutationRollbackReceipt,
) -> Result<String, ContextError> {
    let mut normalized = receipt.clone();
    normalized.binding_digest.clear();
    digest_serializable(&normalized)
}

fn rollback_error(
    code: &str,
    message: impl Into<String>,
    path: Option<String>,
    next_action: impl Into<String>,
) -> ContextError {
    ContextError::new(code, message, DiagnosticStage::Rollback, path, next_action)
}

fn rollback_wrap(code: &str, error: ContextError) -> ContextError {
    rollback_error(
        code,
        error.to_string(),
        error.diagnostic.path,
        "Preserve transaction evidence and reject this rollback.",
    )
}

fn capture_snapshot_bytes(
    project_root: &Path,
    paths: &[String],
) -> Result<BTreeMap<String, Option<Vec<u8>>>, ContextError> {
    paths
        .iter()
        .map(|path| Ok((path.clone(), read_optional_regular(project_root, path)?)))
        .collect()
}

fn verify_expected_before(
    mutation: &ProjectMutation,
    actual: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<(), ContextError> {
    for expected in &mutation.expected_before {
        let actual_digest = actual
            .get(&expected.path)
            .and_then(|bytes| bytes.as_deref())
            .map(content_digest);
        if actual_digest != expected.content_digest {
            return Err(mutation_error(
                "authoring_context.mutation_before_drifted",
                "A declared write path no longer matches its expected before state.",
                Some(expected.path.clone()),
                "Refresh and prepare a new mutation from current bytes.",
            ));
        }
    }
    Ok(())
}

fn expected_after_states(
    mutation: &ProjectMutation,
    before: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<BTreeMap<String, Option<Vec<u8>>>, ContextError> {
    let mut after = before.clone();
    for operation in &mutation.operations {
        match operation {
            ProjectMutationOperation::CreateOrReplace { path, bytes } => {
                after.insert(path.clone(), Some(bytes.clone()));
            }
            ProjectMutationOperation::Delete { path } => {
                after.insert(path.clone(), None);
            }
            ProjectMutationOperation::Move { from, to } => {
                let source = before.get(from).cloned().flatten().ok_or_else(|| {
                    mutation_error(
                        "authoring_context.mutation_move_source_missing",
                        "Mutation move source does not exist in the bound before state.",
                        Some(from.clone()),
                        "Prepare a new mutation with an existing move source.",
                    )
                })?;
                after.insert(from.clone(), None);
                after.insert(to.clone(), Some(source));
            }
        }
    }
    Ok(after)
}

fn persist_before_material(
    project_root: &Path,
    transaction_relative: &str,
    before: &BTreeMap<String, Option<Vec<u8>>>,
    after: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<Vec<JournalSnapshot>, ContextError> {
    let material_root = project_root.join(transaction_relative).join("before");
    fs::create_dir(&material_root).map_err(|e| {
        io_error(
            "authoring_context.mutation_journal_write_failed",
            e,
            &material_root,
        )
    })?;
    let mut snapshots = Vec::with_capacity(before.len());
    for (index, (path, bytes)) in before.iter().enumerate() {
        let material = if let Some(bytes) = bytes {
            let relative = format!("{transaction_relative}/before/{index:04}.bin");
            write_bytes_new(&project_root.join(&relative), bytes)?;
            Some(relative)
        } else {
            None
        };
        snapshots.push(JournalSnapshot {
            path: path.clone(),
            before_digest: bytes.as_deref().map(content_digest),
            after_digest: after
                .get(path)
                .and_then(|value| value.as_deref())
                .map(content_digest),
            before_material_path: material,
        });
    }
    Ok(snapshots)
}

fn apply_operation(
    project_root: &Path,
    operation: &ProjectMutationOperation,
    before: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<(), ContextError> {
    match operation {
        ProjectMutationOperation::CreateOrReplace { path, bytes } => {
            write_project_file_atomic(project_root, path, bytes)
        }
        ProjectMutationOperation::Delete { path } => remove_project_file(project_root, path),
        ProjectMutationOperation::Move { from, to } => {
            let bytes = before.get(from).cloned().flatten().ok_or_else(|| {
                mutation_error(
                    "authoring_context.mutation_move_source_missing",
                    "Mutation move source is missing.",
                    Some(from.clone()),
                    "Prepare a new mutation from current source.",
                )
            })?;
            write_project_file_atomic(project_root, to, &bytes)?;
            remove_project_file(project_root, from)
        }
    }
}

fn compensate_or_require_recovery(
    project_root: &Path,
    before: &BTreeMap<String, Option<Vec<u8>>>,
    after: &BTreeMap<String, Option<Vec<u8>>>,
    journal: &mut MutationJournal,
    journal_path: &Path,
) -> Result<(), ContextError> {
    if let Some(path) = first_state_mismatch(project_root, after)? {
        journal.state = JournalState::RecoveryRequired;
        journal.binding_digest = journal_digest(journal)?;
        write_json_atomic(journal_path, journal)?;
        return Err(mutation_error(
            "authoring_context.mutation_recovery_required",
            "Mutation compensation is unsafe because a write path contains unknown bytes.",
            Some(path),
            "Preserve the transaction and perform explicit recovery.",
        ));
    }
    restore_states(project_root, before)?;
    journal.state = JournalState::RolledBack;
    journal.binding_digest = journal_digest(journal)?;
    write_json_atomic(journal_path, journal)
}

fn restore_states(
    project_root: &Path,
    states: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<(), ContextError> {
    for (path, bytes) in states.iter().rev() {
        if let Some(bytes) = bytes {
            write_project_file_atomic(project_root, path, bytes)?;
        } else {
            remove_project_file(project_root, path)?;
        }
    }
    Ok(())
}

fn first_state_mismatch(
    project_root: &Path,
    expected: &BTreeMap<String, Option<Vec<u8>>>,
) -> Result<Option<String>, ContextError> {
    for (path, bytes) in expected {
        let actual = read_optional_regular(project_root, path)?;
        if actual != *bytes {
            return Ok(Some(path.clone()));
        }
    }
    Ok(None)
}

fn inventory_changes(
    before: &CanonicalSourceInventory,
    after: &CanonicalSourceInventory,
) -> BTreeSet<String> {
    let before = before
        .entries()
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry.content_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .entries()
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry.content_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

fn reject_nonterminal_journal(project_root: &Path) -> Result<(), ContextError> {
    let root = project_root.join(".aife/authoring/transactions");
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&root)
        .map_err(|e| io_error("authoring_context.mutation_journal_read_failed", e, &root))?
    {
        let path = entry
            .map_err(|e| io_error("authoring_context.mutation_journal_read_failed", e, &root))?
            .path()
            .join("journal.json");
        if !path.is_file() {
            continue;
        }
        let journal: MutationJournal =
            serde_json::from_slice(&fs::read(&path).map_err(|e| {
                io_error("authoring_context.mutation_journal_read_failed", e, &path)
            })?)
            .map_err(|error| {
                mutation_error(
                    "authoring_context.mutation_recovery_blocked",
                    format!("Existing mutation journal is invalid: {error}"),
                    Some(path.display().to_string()),
                    "Preserve the journal and perform explicit recovery.",
                )
            })?;
        if matches!(
            journal.state,
            JournalState::Prepared | JournalState::Applying | JournalState::RecoveryRequired
        ) {
            return Err(mutation_error(
                "authoring_context.mutation_recovery_blocked",
                "An existing nonterminal mutation requires recovery.",
                Some(path.display().to_string()),
                "Complete explicit recovery before starting another mutation.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_optional_regular(
    project_root: &Path,
    relative: &str,
) -> Result<Option<Vec<u8>>, ContextError> {
    reject_existing_path_chain(project_root, relative)?;
    let path = project_root.join(relative);
    crate::context::reject_link_or_reparse(&path, DiagnosticStage::Mutation)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(
            "authoring_context.mutation_path_read_failed",
            error,
            &path,
        )),
    }
}

pub(crate) fn write_project_file_atomic(
    project_root: &Path,
    relative: &str,
    bytes: &[u8],
) -> Result<(), ContextError> {
    reject_existing_path_chain(project_root, relative)?;
    let path = project_root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| io_error("authoring_context.mutation_write_failed", e, &path))?;
    }
    crate::context::reject_link_or_reparse(&path, DiagnosticStage::Mutation)?;
    let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("file");
    let temp = path.with_file_name(format!(".{name}.aife-{sequence}.tmp"));
    let backup = path.with_file_name(format!(".{name}.aife-{sequence}.bak"));
    write_bytes_new(&temp, bytes)?;
    let existed = path.exists();
    if existed {
        fs::rename(&path, &backup)
            .map_err(|e| io_error("authoring_context.mutation_write_failed", e, &path))?;
    }
    if let Err(error) = fs::rename(&temp, &path) {
        if existed {
            let _ = fs::rename(&backup, &path);
        }
        let _ = fs::remove_file(&temp);
        return Err(io_error(
            "authoring_context.mutation_write_failed",
            error,
            &path,
        ));
    }
    if existed {
        fs::remove_file(&backup)
            .map_err(|e| io_error("authoring_context.mutation_write_failed", e, &path))?;
    }
    Ok(())
}

fn remove_project_file(project_root: &Path, relative: &str) -> Result<(), ContextError> {
    reject_existing_path_chain(project_root, relative)?;
    let path = project_root.join(relative);
    crate::context::reject_link_or_reparse(&path, DiagnosticStage::Mutation)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(
            "authoring_context.mutation_remove_failed",
            error,
            &path,
        )),
    }
}

fn reject_existing_path_chain(project_root: &Path, relative: &str) -> Result<(), ContextError> {
    let mut current = project_root.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for component in components {
        let Component::Normal(component) = component else {
            return Err(path_error(relative));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                let _ = metadata;
                crate::context::reject_link_or_reparse(&current, DiagnosticStage::Mutation)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => {
                return Err(io_error(
                    "authoring_context.source_metadata_failed",
                    error,
                    &current,
                ));
            }
        }
    }
    Ok(())
}

fn write_bytes_new(path: &Path, bytes: &[u8]) -> Result<(), ContextError> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| io_error("authoring_context.mutation_journal_write_failed", e, path))?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|e| io_error("authoring_context.mutation_journal_write_failed", e, path))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), ContextError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| io_error("authoring_context.mutation_journal_write_failed", e, path))?;
    }
    let bytes = serde_json::to_vec(value).map_err(|error| {
        mutation_error(
            "authoring_context.mutation_journal_encode_failed",
            format!("Mutation control record cannot be encoded: {error}"),
            Some(path.display().to_string()),
            "Inspect the mutation control schema implementation.",
        )
    })?;
    let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("json.{sequence}.tmp"));
    let backup = path.with_extension(format!("json.{sequence}.bak"));
    write_bytes_new(&temp, &bytes)?;
    let existed = path.exists();
    if existed {
        fs::rename(path, &backup)
            .map_err(|e| io_error("authoring_context.mutation_journal_write_failed", e, path))?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        if existed {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temp);
        return Err(io_error(
            "authoring_context.mutation_journal_write_failed",
            error,
            path,
        ));
    }
    if existed {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

fn journal_digest(journal: &MutationJournal) -> Result<String, ContextError> {
    let mut normalized = journal.clone();
    normalized.binding_digest.clear();
    digest_serializable(&normalized)
}

fn receipt_digest(receipt: &ProjectMutationReceipt) -> Result<String, ContextError> {
    let mut normalized = receipt.clone();
    normalized.binding_digest.clear();
    digest_serializable(&normalized)
}

fn digest_serializable(value: &impl Serialize) -> Result<String, ContextError> {
    serde_json::to_vec(value)
        .map(|bytes| content_digest(&bytes))
        .map_err(|error| {
            mutation_error(
                "authoring_context.mutation_binding_encode_failed",
                format!("Mutation binding cannot be encoded: {error}"),
                None,
                "Inspect the mutation binding implementation.",
            )
        })
}

fn content_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn io_error(code: &str, error: std::io::Error, path: &Path) -> ContextError {
    mutation_error(
        code,
        error.to_string(),
        Some(path.display().to_string()),
        "Preserve transaction evidence and inspect filesystem access.",
    )
}

fn control_error(reason: String, project_root: &Path) -> ContextError {
    mutation_error(
        "authoring_context.mutation_commit_interrupted",
        format!("Mutation commit was interrupted: {reason}"),
        Some(project_root.display().to_string()),
        "Inspect the cause and prepare a fresh mutation.",
    )
}

pub(crate) fn validate_digest(value: &str, label: &str) -> Result<(), ContextError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(mutation_error(
            "authoring_context.mutation_digest_invalid",
            format!("ProjectMutation {label} is not a canonical sha256 digest."),
            None,
            "Regenerate the mutation from sealed validation evidence.",
        ))
    }
}

fn validate_token(value: &str, label: &str) -> Result<(), ContextError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(mutation_error(
            "authoring_context.mutation_identity_invalid",
            format!("ProjectMutation {label} must use 1-128 portable ASCII characters."),
            None,
            "Use letters, digits, '.', '-' or '_'.",
        ))
    }
}

fn validate_path_set(paths: &[String], label: &str) -> Result<BTreeSet<String>, ContextError> {
    let mut result = BTreeSet::new();
    for raw_path in paths {
        let path = validate_relative_path(raw_path)?;
        if !result.insert(path.clone()) {
            return Err(mutation_error(
                "authoring_context.mutation_path_duplicate",
                format!("ProjectMutation declared {label} set contains duplicate paths."),
                Some(path),
                "Declare each canonical path once.",
            ));
        }
    }
    let mut folded = BTreeSet::new();
    for path in &result {
        if !folded.insert(path.to_ascii_lowercase()) {
            return Err(mutation_error(
                "authoring_context.mutation_path_case_collision",
                "ProjectMutation paths collide on a case-insensitive filesystem.",
                Some(path.clone()),
                "Use one portable spelling for each path.",
            ));
        }
    }
    Ok(result)
}

pub(crate) fn validate_relative_path(raw_path: &str) -> Result<String, ContextError> {
    if raw_path.is_empty() || raw_path.contains('\\') || raw_path.starts_with('/') {
        return Err(path_error(raw_path));
    }
    let mut parts = Vec::new();
    for component in Path::new(raw_path).components() {
        let Component::Normal(value) = component else {
            return Err(path_error(raw_path));
        };
        let value = value.to_str().ok_or_else(|| path_error(raw_path))?;
        if value.is_empty() || value.ends_with([' ', '.']) || value.contains(['/', '\\']) {
            return Err(path_error(raw_path));
        }
        parts.push(value);
    }
    let normalized = parts.join("/");
    if normalized != raw_path || normalized.len() > 512 {
        return Err(path_error(raw_path));
    }
    Ok(normalized)
}

fn is_control_path(path: &str) -> bool {
    path.split('/').next().is_some_and(|first| {
        first.eq_ignore_ascii_case(".aife")
            || first.eq_ignore_ascii_case(".aife-candidates")
            || first.eq_ignore_ascii_case(".git")
    })
}

fn path_error(path: &str) -> ContextError {
    mutation_error(
        "authoring_context.mutation_path_invalid",
        "ProjectMutation path is not canonical portable relative syntax.",
        Some(path.to_string()),
        "Use normalized UTF-8 project-relative paths without '.', '..', roots or backslashes.",
    )
}

pub(crate) fn mutation_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<String>,
    next_action: impl Into<String>,
) -> ContextError {
    ContextError::new(code, message, DiagnosticStage::Mutation, path, next_action)
}
