use super::model::*;
use crate::ProjectWriteScope;
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const PROJECT_JOURNAL_DIRECTORY: &str = "Library/ProjectIntent";
pub(crate) const PROJECT_JOURNAL_PATH: &str = "Library/ProjectIntent/journal.json";

#[derive(Clone)]
pub(crate) enum ProjectIntentStorage {
    InMemory,
    LocalDraft(PathBuf),
    Project { write_scope: ProjectWriteScope },
}

impl std::fmt::Debug for ProjectIntentStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InMemory => formatter.write_str("InMemory"),
            Self::LocalDraft(path) => formatter.debug_tuple("LocalDraft").field(path).finish(),
            Self::Project { .. } => formatter.write_str("Project"),
        }
    }
}

pub(crate) fn load_local_draft(
    path: &Path,
) -> Result<ProjectIntentJournalDocument, ProjectIntentWorkflowError> {
    if !path.exists() {
        return finalize_journal(ProjectIntentJournalDocument::default());
    }
    let bytes = fs::read(path).map_err(|error| {
        workflow_error(
            "project_intent.draft_read_failed",
            format!("Could not read pre-project intent draft: {error}"),
            "Keep the draft file and retry from a readable Launcher storage path.",
        )
    })?;
    decode_and_validate(&bytes)
}

pub(crate) fn load_project_journal(
    write_scope: &ProjectWriteScope,
) -> Result<ProjectIntentJournalDocument, ProjectIntentWorkflowError> {
    let exists = write_scope
        .try_exists(PROJECT_JOURNAL_PATH)
        .map_err(|error| {
            workflow_error(
                "project_intent.project_journal_probe_failed",
                error.to_string(),
                "Reopen the project after resolving its Library containment state.",
            )
        })?;
    if !exists {
        return finalize_journal(ProjectIntentJournalDocument::default());
    }
    let bytes = write_scope.read(PROJECT_JOURNAL_PATH).map_err(|error| {
        workflow_error(
            "project_intent.project_journal_read_failed",
            error.to_string(),
            "Do not discard the journal; repair or restore the exact Library file.",
        )
    })?;
    decode_and_validate(&bytes)
}

pub(crate) fn persist(
    storage: &ProjectIntentStorage,
    journal: &ProjectIntentJournalDocument,
) -> Result<(), ProjectIntentWorkflowError> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(|error| {
        workflow_error(
            "project_intent.journal_serialize_failed",
            error.to_string(),
            "Fix the journal schema before attempting another mutation.",
        )
    })?;
    match storage {
        ProjectIntentStorage::InMemory => Ok(()),
        ProjectIntentStorage::LocalDraft(path) => write_local_atomic(path, &bytes),
        ProjectIntentStorage::Project { write_scope } => {
            write_scope
                .create_dir_all(PROJECT_JOURNAL_DIRECTORY)
                .map_err(|error| {
                    workflow_error(
                        "project_intent.project_journal_directory_failed",
                        error.to_string(),
                        "Resolve project Library write containment and retry.",
                    )
                })?;
            write_scope
                .write_atomic(PROJECT_JOURNAL_PATH, &bytes)
                .map_err(|error| {
                    workflow_error(
                        "project_intent.project_journal_write_failed",
                        error.to_string(),
                        "Keep the in-memory journal and retry the same command after storage recovers.",
                    )
                })?;
            Ok(())
        }
    }
}

pub(crate) fn append_record(
    journal: &mut ProjectIntentJournalDocument,
    storage: &ProjectIntentStorage,
    command_id: &str,
    record: ProjectIntentJournalRecord,
) -> Result<(u64, bool), ProjectIntentWorkflowError> {
    validate_command_id(command_id)?;
    if let Some(entry) = journal
        .entries
        .iter()
        .find(|entry| entry.command_id == command_id)
    {
        return Ok((entry.revision, true));
    }
    let revision = journal.revision.saturating_add(1);
    let mut entry = ProjectIntentJournalEntry {
        revision,
        command_id: command_id.to_string(),
        occurred_at: timestamp_string(),
        record,
        entry_digest: String::new(),
    };
    entry.entry_digest = digest_without_field(&entry, "entryDigest")?;
    let mut next = journal.clone();
    if let ProjectIntentJournalRecord::ProjectAttached(binding) = &entry.record {
        next.project_binding = Some(binding.clone());
    }
    next.entries.push(entry);
    next.revision = revision;
    next.journal_digest.clear();
    next.journal_digest = digest_without_field(&next, "journalDigest")?;
    persist(storage, &next)?;
    *journal = next;
    Ok((revision, false))
}

pub(crate) fn finalize_journal(
    mut journal: ProjectIntentJournalDocument,
) -> Result<ProjectIntentJournalDocument, ProjectIntentWorkflowError> {
    journal.journal_digest.clear();
    journal.journal_digest = digest_without_field(&journal, "journalDigest")?;
    Ok(journal)
}

pub(crate) fn rebuild_snapshot(
    journal: &ProjectIntentJournalDocument,
) -> Result<ProjectIntentSnapshot, ProjectIntentWorkflowError> {
    validate_journal(journal)?;
    let mut events = BTreeMap::<String, IntentEvent>::new();
    let mut work_items = BTreeMap::<String, WorkItem>::new();
    let mut diagnoses = BTreeMap::<String, ProjectDiagnosisSession>::new();
    let mut active_proposal = None;
    let mut active_approval = None;
    let mut active_run = None;
    let mut processed_commands = BTreeMap::new();

    for entry in &journal.entries {
        processed_commands.insert(entry.command_id.clone(), entry.revision);
        match &entry.record {
            ProjectIntentJournalRecord::IntentCaptured(event) => {
                events.insert(event.event_id.clone(), event.clone());
            }
            ProjectIntentJournalRecord::WorkItemChanged(work_item) => {
                work_items.insert(work_item.work_item_id.clone(), work_item.clone());
            }
            ProjectIntentJournalRecord::WorkItemsChanged(changed) => {
                for work_item in changed {
                    work_items.insert(work_item.work_item_id.clone(), work_item.clone());
                }
            }
            ProjectIntentJournalRecord::DiagnosisChanged(diagnosis) => {
                diagnoses.insert(diagnosis.diagnosis_id.clone(), diagnosis.clone());
            }
            ProjectIntentJournalRecord::ChangeSetPrepared {
                proposal,
                work_items: changed,
            } => {
                for work_item in changed {
                    work_items.insert(work_item.work_item_id.clone(), work_item.clone());
                }
                active_proposal = Some(proposal.clone());
                active_approval = None;
            }
            ProjectIntentJournalRecord::RunAuthorized {
                approval,
                run,
                work_items: changed,
            } => {
                for work_item in changed {
                    work_items.insert(work_item.work_item_id.clone(), work_item.clone());
                }
                active_approval = Some(approval.clone());
                active_run = Some(run.clone());
            }
            ProjectIntentJournalRecord::RunChanged {
                run,
                work_items: changed,
            } => {
                for work_item in changed {
                    work_items.insert(work_item.work_item_id.clone(), work_item.clone());
                }
                active_run = Some(run.clone());
            }
            ProjectIntentJournalRecord::ProjectAttached(_) => {}
        }
    }

    let work_item_summaries = work_items
        .values()
        .map(|work_item| WorkItemSummary {
            work_item_id: work_item.work_item_id.clone(),
            kind: work_item.kind,
            title: work_item.title.clone(),
            status: work_item.status,
            ready: work_item_ready(work_item, &work_items),
            revision: work_item.revision,
            work_item_digest: work_item.work_item_digest.clone(),
        })
        .collect::<Vec<_>>();
    let active_diagnoses = diagnoses
        .values()
        .filter(|diagnosis| diagnosis.state != DiagnosisState::Closed)
        .cloned()
        .collect::<Vec<_>>();
    let active_diagnosis_summaries = active_diagnoses
        .iter()
        .map(|diagnosis| DiagnosisSummary {
            diagnosis_id: diagnosis.diagnosis_id.clone(),
            work_item_id: diagnosis.work_item_id.clone(),
            state: diagnosis.state,
            diagnosis_digest: diagnosis.diagnosis_digest.clone(),
        })
        .collect();
    let referenced_events = work_items
        .values()
        .flat_map(|item| item.source_event_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let pending_normalization_event_ids = events
        .keys()
        .filter(|event_id| !referenced_events.contains(*event_id))
        .cloned()
        .collect();

    Ok(ProjectIntentSnapshot {
        schema_version: PROJECT_INTENT_SNAPSHOT_SCHEMA_VERSION.to_string(),
        checkpoint_id: format!("checkpoint-{:016}", journal.revision),
        journal_revision: journal.revision,
        journal_digest: journal.journal_digest.clone(),
        project_binding: journal.project_binding.clone(),
        intent_events: events.into_values().collect(),
        work_items: work_items.into_values().collect(),
        work_item_summaries,
        active_diagnoses,
        active_diagnosis_summaries,
        active_proposal,
        active_approval,
        active_run,
        pending_normalization_event_ids,
        processed_commands,
        diagnostics: Vec::new(),
    })
}

pub(crate) fn validate_journal(
    journal: &ProjectIntentJournalDocument,
) -> Result<(), ProjectIntentWorkflowError> {
    if journal.schema_version != PROJECT_INTENT_JOURNAL_SCHEMA_VERSION {
        return Err(workflow_error(
            "project_intent.journal_schema_unsupported",
            format!("Unsupported journal schema {}.", journal.schema_version),
            "Migrate the journal explicitly before opening it.",
        ));
    }
    if journal.revision != journal.entries.len() as u64 {
        return Err(workflow_error(
            "project_intent.journal_revision_invalid",
            "Journal revision does not match its append-only entry count.",
            "Restore the last intact journal checkpoint.",
        ));
    }
    let mut commands = BTreeSet::new();
    let mut active_proposal: Option<&ChangeSetProposal> = None;
    let mut active_approval: Option<&ChangeSetApproval> = None;
    let mut active_run: Option<&ProjectProductionRun> = None;
    for (index, entry) in journal.entries.iter().enumerate() {
        if entry.revision != index as u64 + 1 {
            return Err(workflow_error(
                "project_intent.journal_sequence_invalid",
                "Journal entries are not in a continuous revision sequence.",
                "Restore the last intact journal checkpoint.",
            ));
        }
        if !commands.insert(entry.command_id.as_str()) {
            return Err(workflow_error(
                "project_intent.command_replay_conflict",
                "A command id appears more than once in the journal.",
                "Keep only the original append result.",
            ));
        }
        if digest_without_field(entry, "entryDigest")? != entry.entry_digest {
            return Err(workflow_error(
                "project_intent.entry_digest_mismatch",
                format!("Journal entry {} failed digest validation.", entry.revision),
                "Restore the exact untampered journal entry.",
            ));
        }
        match &entry.record {
            ProjectIntentJournalRecord::IntentCaptured(event) => {
                validate_bound_digest(event, "contentDigest", &event.content_digest)?;
            }
            ProjectIntentJournalRecord::WorkItemChanged(item) => validate_work_item(item)?,
            ProjectIntentJournalRecord::WorkItemsChanged(items) => {
                for item in items {
                    validate_work_item(item)?;
                }
            }
            ProjectIntentJournalRecord::DiagnosisChanged(diagnosis) => {
                validate_bound_digest(diagnosis, "diagnosisDigest", &diagnosis.diagnosis_digest)?;
            }
            ProjectIntentJournalRecord::ChangeSetPrepared {
                proposal,
                work_items,
            } => {
                validate_bound_digest(proposal, "proposalDigest", &proposal.proposal_digest)?;
                for item in work_items {
                    validate_work_item(item)?;
                }
                for binding in &proposal.selected_work_item_revisions {
                    if !work_items.iter().any(|item| {
                        item.work_item_id == binding.work_item_id
                            && item.revision == binding.revision
                            && item.work_item_digest == binding.work_item_digest
                    }) {
                        return Err(workflow_error(
                            "project_intent.proposal_work_item_binding_invalid",
                            "ChangeSetProposal does not bind its selected WorkItem revisions.",
                            "Restore or reprepare the exact selected WorkItems.",
                        ));
                    }
                }
                active_proposal = Some(proposal);
                active_approval = None;
            }
            ProjectIntentJournalRecord::RunAuthorized {
                approval,
                run,
                work_items,
            } => {
                validate_bound_digest(approval, "approvalDigest", &approval.approval_digest)?;
                for item in work_items {
                    validate_work_item(item)?;
                }
                let proposal = active_proposal.ok_or_else(|| {
                    workflow_error(
                        "project_intent.authorization_proposal_missing",
                        "Run authorization has no prior active ChangeSetProposal.",
                        "Restore the proposal and approval sequence.",
                    )
                })?;
                if approval.proposal_digest != proposal.proposal_digest
                    || run.proposal_id != proposal.proposal_id
                    || run.change_set_approval_digest != approval.approval_digest
                {
                    return Err(workflow_error(
                        "project_intent.authorization_binding_invalid",
                        "Run authorization does not bind its proposal and approval digests.",
                        "Restore the exact authorization record.",
                    ));
                }
                active_approval = Some(approval);
                active_run = Some(run);
            }
            ProjectIntentJournalRecord::RunChanged { run, work_items } => {
                for item in work_items {
                    validate_work_item(item)?;
                }
                let approval = active_approval.ok_or_else(|| {
                    workflow_error(
                        "project_intent.run_approval_missing",
                        "Run progress has no active approval record.",
                        "Restore the exact approval and run sequence.",
                    )
                })?;
                if run.change_set_approval_digest != approval.approval_digest
                    || active_run.is_some_and(|prior| prior.run_id != run.run_id)
                {
                    return Err(workflow_error(
                        "project_intent.run_binding_invalid",
                        "Run progress changed its run or approval identity.",
                        "Restore the exact append-only run record.",
                    ));
                }
                active_run = Some(run);
            }
            ProjectIntentJournalRecord::ProjectAttached(_) => {}
        }
    }
    if digest_without_field(journal, "journalDigest")? != journal.journal_digest {
        return Err(workflow_error(
            "project_intent.journal_digest_mismatch",
            "Journal digest does not match its canonical content.",
            "Restore the exact untampered journal document.",
        ));
    }
    Ok(())
}

fn validate_work_item(item: &WorkItem) -> Result<(), ProjectIntentWorkflowError> {
    if super::work_item_semantic_digest(item)? != item.work_item_digest {
        return Err(workflow_error(
            "project_intent.work_item_digest_mismatch",
            format!(
                "WorkItem {} failed semantic digest validation.",
                item.work_item_id
            ),
            "Restore the exact WorkItem revision or create a new revision.",
        ));
    }
    Ok(())
}

fn validate_bound_digest<T: Serialize>(
    value: &T,
    field: &str,
    expected: &str,
) -> Result<(), ProjectIntentWorkflowError> {
    if digest_without_field(value, field)? != expected {
        return Err(workflow_error(
            "project_intent.bound_digest_mismatch",
            format!("Workflow object failed its {field} binding."),
            "Restore the exact untampered object or create a new revision.",
        ));
    }
    Ok(())
}

pub(crate) fn digest_record<T: Serialize>(value: &T) -> Result<String, ProjectIntentWorkflowError> {
    let value = serde_json::to_value(value).map_err(|error| {
        workflow_error(
            "project_intent.digest_serialize_failed",
            error.to_string(),
            "Fix the schema value before computing its binding digest.",
        )
    })?;
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            workflow_error(
                "project_intent.digest_canonicalize_failed",
                error.to_string(),
                "Fix the schema value before computing its binding digest.",
            )
        })
}

pub(crate) fn timestamp_string() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("unix-ms-{millis}")
}

fn decode_and_validate(
    bytes: &[u8],
) -> Result<ProjectIntentJournalDocument, ProjectIntentWorkflowError> {
    let journal =
        serde_json::from_slice::<ProjectIntentJournalDocument>(bytes).map_err(|error| {
            workflow_error(
                "project_intent.journal_parse_failed",
                error.to_string(),
                "Repair or restore the strict journal JSON before continuing.",
            )
        })?;
    validate_journal(&journal)?;
    Ok(journal)
}

fn write_local_atomic(path: &Path, bytes: &[u8]) -> Result<(), ProjectIntentWorkflowError> {
    let parent = path.parent().ok_or_else(|| {
        workflow_error(
            "project_intent.draft_path_invalid",
            "Pre-project draft path has no parent directory.",
            "Choose an explicit Launcher-local draft file path.",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        workflow_error(
            "project_intent.draft_directory_failed",
            error.to_string(),
            "Choose a writable Launcher-local draft directory.",
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            workflow_error(
                "project_intent.draft_path_invalid",
                "Pre-project draft file name is not valid UTF-8.",
                "Choose a normal Launcher-local draft file name.",
            )
        })?;
    let temporary = parent.join(format!(".{file_name}.tmp"));
    fs::write(&temporary, bytes).map_err(|error| {
        workflow_error(
            "project_intent.draft_write_failed",
            error.to_string(),
            "Keep the in-memory draft and retry after storage recovers.",
        )
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            workflow_error(
                "project_intent.draft_replace_failed",
                error.to_string(),
                "Keep the temporary draft and resolve the locked destination.",
            )
        })?;
    }
    fs::rename(&temporary, path).map_err(|error| {
        workflow_error(
            "project_intent.draft_publish_failed",
            error.to_string(),
            "Keep the temporary draft and retry atomic publication.",
        )
    })
}

fn digest_without_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String, ProjectIntentWorkflowError> {
    let mut value = serde_json::to_value(value).map_err(|error| {
        workflow_error(
            "project_intent.digest_serialize_failed",
            error.to_string(),
            "Fix the schema value before computing its binding digest.",
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        workflow_error(
            "project_intent.digest_shape_invalid",
            "Digest-bound schema must serialize as an object.",
            "Use a strict object schema for workflow records.",
        )
    })?;
    object.insert(field.to_string(), serde_json::Value::String(String::new()));
    canonical_json_bytes(&value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            workflow_error(
                "project_intent.digest_canonicalize_failed",
                error.to_string(),
                "Fix the schema value before computing its binding digest.",
            )
        })
}

fn work_item_ready(item: &WorkItem, all: &BTreeMap<String, WorkItem>) -> bool {
    if item.status != WorkItemStatus::Ready || !item.open_questions.is_empty() {
        return false;
    }
    item.relationship_refs
        .iter()
        .filter(|relation| relation.kind == WorkItemRelationshipKind::DependsOn)
        .all(|relation| {
            all.get(&relation.target_work_item_id)
                .is_some_and(|target| {
                    matches!(target.status, WorkItemStatus::Ready | WorkItemStatus::Done)
                        && target.open_questions.is_empty()
                })
        })
}

fn validate_command_id(command_id: &str) -> Result<(), ProjectIntentWorkflowError> {
    if command_id.trim().is_empty() {
        return Err(workflow_error(
            "project_intent.command_id_missing",
            "Workflow mutation requires a stable command id.",
            "Generate one command id and reuse it only for exact retries.",
        ));
    }
    Ok(())
}

fn workflow_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> ProjectIntentWorkflowError {
    ProjectIntentWorkflowError::new(code, message, next_action)
}
