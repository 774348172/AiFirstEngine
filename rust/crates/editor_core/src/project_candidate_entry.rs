use crate::{
    CandidateProjectRevisionStore, ControlledSourcePatch, ControlledSourcePatchApplyReceipt,
    ControlledSourcePatchApplyRequest, ControlledSourcePatchApproval,
    ControlledSourcePatchCandidate, ControlledSourcePatchError,
    ControlledSourcePatchRollbackReceipt, ControlledSourcePatchValidationReport,
    ControlledSourcePatchValidationRequest, EditorSession, PatchApplyReport, PatchApplyStatus,
    PatchValidationReport, PatchValidator, ProjectAssetImport, ProjectAssetImportApplyReceipt,
    ProjectAssetImportApplyRequest, ProjectAssetImportApproval, ProjectAssetImportCandidate,
    ProjectAssetImportError, ProjectAssetImportPrepareRequest, ProjectAssetImportRollbackReceipt,
    ProjectAssetImportValidationReport, ProjectPatchDocument, ProjectPatchImportRequest,
    ProjectPatchImportService, ProjectPatchImportSourceKind, ProjectPatchLlmContextSnapshot,
};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION: &str = "project-candidate-envelope.v1";
pub const PROJECT_CANDIDATE_SCHEMA_VERSION: &str = "project-candidate.v1";
pub const PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION: &str =
    "project-candidate-project-binding.v1";
pub const PROJECT_CANDIDATE_VALIDATION_REPORT_SCHEMA_VERSION: &str =
    "project-candidate-validation-report.v1";
pub const PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION: &str = "project-candidate-approval.v1";
pub const PROJECT_CANDIDATE_APPLY_RECEIPT_SCHEMA_VERSION: &str =
    "project-candidate-apply-receipt.v1";
pub const PROJECT_CANDIDATE_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str =
    "project-candidate-rollback-receipt.v1";

const MAX_CANDIDATE_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_CANDIDATE_ID_CHARS: usize = 128;
const MAX_SOURCE_LABEL_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCandidateSourceKind {
    BuiltInProvider,
    ImportedCodex,
    ImportedFile,
    TestFixture,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "payloadKind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProjectCandidatePayload {
    ProjectPatch(ProjectPatchDocument),
    ControlledSourcePatch {
        request: crate::ControlledSourcePatchPrepareRequest,
    },
    AssetImport {
        request: ProjectAssetImportPrepareRequest,
        expected_source_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateEnvelope {
    pub schema_version: String,
    pub candidate_id: String,
    pub source_kind: ProjectCandidateSourceKind,
    pub source_label: String,
    pub target_project_id: String,
    pub expected_base_project_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_patch_context_hash: Option<String>,
    pub payload: ProjectCandidatePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateProjectBinding {
    pub schema_version: String,
    pub project_id: String,
    pub project_root: String,
    pub project_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidatePrepareRequest {
    pub envelope: ProjectCandidateEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payloadKind", content = "payload", rename_all = "snake_case")]
pub enum PreparedProjectCandidatePayload {
    ProjectPatch {
        patch: ProjectPatchDocument,
    },
    ControlledSourcePatch {
        candidate: ControlledSourcePatchCandidate,
    },
    AssetImport {
        candidate: ProjectAssetImportCandidate,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidate {
    pub schema_version: String,
    pub envelope: ProjectCandidateEnvelope,
    pub project_binding: ProjectCandidateProjectBinding,
    pub source_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub envelope_digest: String,
    pub payload_digest: String,
    pub candidate_digest: String,
    pub prepared_payload: PreparedProjectCandidatePayload,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectCandidateValidationContext {
    pub controlled_source_patch: Option<ControlledSourcePatchValidationRequest>,
    pub cancellation: Option<runtime_cli::BoundedChildProcessCancellation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payloadKind", content = "payload", rename_all = "snake_case")]
pub enum ProjectCandidateValidationPayload {
    ProjectPatch {
        report: PatchValidationReport,
    },
    ControlledSourcePatch {
        report: ControlledSourcePatchValidationReport,
    },
    AssetImport {
        report: ProjectAssetImportValidationReport,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCandidateValidationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateValidationReport {
    pub schema_version: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub status: ProjectCandidateValidationStatus,
    pub validation_digest: String,
    pub payload_validation: ProjectCandidateValidationPayload,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateApproval {
    pub schema_version: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub validation_digest: String,
    pub approved_by: String,
    pub allow_replace: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payloadKind", content = "payload", rename_all = "snake_case")]
pub enum ProjectCandidateAppliedPayload {
    ProjectPatch {
        patch_id: String,
        report: PatchApplyReport,
    },
    ControlledSourcePatch {
        receipt: ControlledSourcePatchApplyReceipt,
    },
    AssetImport {
        receipt: ProjectAssetImportApplyReceipt,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateApplyReceipt {
    pub schema_version: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub validation_digest: String,
    pub project_id: String,
    pub project_root: String,
    pub before_project_digest: String,
    pub applied_project_digest: String,
    pub applied_payload: ProjectCandidateAppliedPayload,
    pub receipt_binding_digest: String,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payloadKind", content = "payload", rename_all = "snake_case")]
pub enum ProjectCandidateRolledBackPayload {
    ProjectPatch {
        report: PatchApplyReport,
    },
    ControlledSourcePatch {
        receipt: ControlledSourcePatchRollbackReceipt,
    },
    AssetImport {
        receipt: ProjectAssetImportRollbackReceipt,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateRollbackReceipt {
    pub schema_version: String,
    pub candidate_id: String,
    pub restored_project_digest: String,
    pub replaced_project_digest: String,
    pub rolled_back_payload: ProjectCandidateRolledBackPayload,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub next_action: String,
}

impl ProjectCandidateError {
    fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<&Path>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: path.map(|value| value.display().to_string()),
            next_action: next_action.into(),
        }
    }
}

impl std::fmt::Display for ProjectCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectCandidateError {}

pub struct ProjectCandidateEntry;

impl ProjectCandidateEntry {
    pub fn inspect_project_binding(
        session: &EditorSession,
    ) -> Result<ProjectCandidateProjectBinding, ProjectCandidateError> {
        let project = session.active_project_session().ok_or_else(|| {
            ProjectCandidateError::new(
                "project_candidate_entry.no_active_project",
                "A provider-independent candidate requires an active project.",
                None,
                "Open or create the target project before preparing the candidate.",
            )
        })?;
        let project_root = project.project_root.canonicalize().map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.project_root_invalid",
                format!("Active project root cannot be canonicalized: {error}"),
                Some(&project.project_root),
                "Reopen the project from a valid regular directory.",
            )
        })?;
        let project_digest = CandidateProjectRevisionStore::project_digest(&project_root)
            .map_err(candidate_revision_error)?;
        Ok(ProjectCandidateProjectBinding {
            schema_version: PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION.to_string(),
            project_id: project.manifest.project_id.clone(),
            project_root: project_root.display().to_string(),
            project_digest,
        })
    }

    pub fn project_patch_envelope(
        session: &EditorSession,
        candidate_id: impl Into<String>,
        source_kind: ProjectCandidateSourceKind,
        source_label: impl Into<String>,
        patch: ProjectPatchDocument,
    ) -> Result<ProjectCandidateEnvelope, ProjectCandidateError> {
        let binding = Self::inspect_project_binding(session)?;
        Ok(ProjectCandidateEnvelope {
            schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
            candidate_id: candidate_id.into(),
            source_kind,
            source_label: source_label.into(),
            target_project_id: binding.project_id,
            expected_base_project_digest: binding.project_digest,
            project_patch_context_hash: Some(
                ProjectPatchLlmContextSnapshot::capture(session).context_hash,
            ),
            payload: ProjectCandidatePayload::ProjectPatch(patch),
        })
    }

    pub fn from_json_string(
        session: &EditorSession,
        raw_json: &str,
    ) -> Result<ProjectCandidate, ProjectCandidateError> {
        if raw_json.len() > MAX_CANDIDATE_INPUT_BYTES {
            return Err(input_too_large_error());
        }
        let envelope =
            serde_json::from_str::<ProjectCandidateEnvelope>(raw_json).map_err(|error| {
                ProjectCandidateError::new(
                    "project_candidate_entry.envelope_parse_failed",
                    format!("Candidate envelope is not strict valid JSON: {error}"),
                    None,
                    "Fix the envelope schema and remove unknown fields before retrying.",
                )
            })?;
        Self::prepare_with_source_digest(
            session,
            envelope,
            sha256_prefixed(raw_json.as_bytes()),
            None,
        )
    }

    pub fn from_file(
        session: &EditorSession,
        path: impl AsRef<Path>,
    ) -> Result<ProjectCandidate, ProjectCandidateError> {
        let path = path.as_ref();
        let bytes = read_bounded_regular_file(path)?;
        let raw_json = std::str::from_utf8(&bytes).map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.file_not_utf8",
                format!("Candidate file is not valid UTF-8: {error}"),
                Some(path),
                "Save the candidate as UTF-8 JSON and retry.",
            )
        })?;
        let envelope =
            serde_json::from_str::<ProjectCandidateEnvelope>(raw_json).map_err(|error| {
                ProjectCandidateError::new(
                    "project_candidate_entry.envelope_parse_failed",
                    format!("Candidate file is not strict valid envelope JSON: {error}"),
                    Some(path),
                    "Fix the envelope schema and remove unknown fields before retrying.",
                )
            })?;
        if envelope.source_kind != ProjectCandidateSourceKind::ImportedFile {
            return Err(ProjectCandidateError::new(
                "project_candidate_entry.file_source_kind_mismatch",
                "A file candidate must declare sourceKind imported_file.",
                Some(path),
                "Set sourceKind to imported_file and regenerate the candidate digest.",
            ));
        }
        let canonical_path = path.canonicalize().map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.file_canonicalize_failed",
                format!("Candidate file cannot be canonicalized: {error}"),
                Some(path),
                "Provide a stable regular candidate file.",
            )
        })?;
        Self::prepare_with_source_digest(
            session,
            envelope,
            sha256_prefixed(&bytes),
            Some(canonical_path.display().to_string()),
        )
    }

    pub fn prepare(
        session: &EditorSession,
        request: ProjectCandidatePrepareRequest,
    ) -> Result<ProjectCandidate, ProjectCandidateError> {
        let source_digest = digest_serializable(&request.envelope, "candidate envelope")?;
        Self::prepare_with_source_digest(session, request.envelope, source_digest, None)
    }

    pub fn prepare_with_source_file(
        session: &EditorSession,
        request: ProjectCandidatePrepareRequest,
        source_path: impl AsRef<Path>,
    ) -> Result<ProjectCandidate, ProjectCandidateError> {
        let source_path = source_path.as_ref();
        let bytes = read_bounded_regular_file(source_path)?;
        let canonical_path = source_path.canonicalize().map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.file_canonicalize_failed",
                format!("Candidate source file cannot be canonicalized: {error}"),
                Some(source_path),
                "Provide a stable regular candidate source file.",
            )
        })?;
        Self::prepare_with_source_digest(
            session,
            request.envelope,
            sha256_prefixed(&bytes),
            Some(canonical_path.display().to_string()),
        )
    }

    fn prepare_with_source_digest(
        session: &EditorSession,
        envelope: ProjectCandidateEnvelope,
        source_digest: String,
        source_path: Option<String>,
    ) -> Result<ProjectCandidate, ProjectCandidateError> {
        validate_envelope(&envelope)?;
        let binding = Self::inspect_project_binding(session)?;
        validate_project_binding(&envelope, &binding)?;
        let envelope_digest = digest_serializable(&envelope, "candidate envelope")?;
        let payload_digest = digest_serializable(&envelope.payload, "candidate payload")?;
        let prepared_payload = match &envelope.payload {
            ProjectCandidatePayload::ProjectPatch(patch) => {
                let actual_context = ProjectPatchLlmContextSnapshot::capture(session).context_hash;
                if envelope.project_patch_context_hash.as_deref() != Some(actual_context.as_str()) {
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.context_mismatch",
                        "ProjectPatch candidate context does not match the active editor context.",
                        None,
                        "Regenerate or reimport the candidate against the current project context.",
                    ));
                }
                let import_request = ProjectPatchImportRequest {
                    schema_version: crate::PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION.to_string(),
                    source_kind: import_source_kind(envelope.source_kind),
                    source_label: envelope.source_label.clone(),
                    project_root: Some(binding.project_root.clone()),
                    raw_json: Some(serde_json::to_string(patch).map_err(|error| {
                        ProjectCandidateError::new(
                            "project_candidate_entry.payload_serialize_failed",
                            format!("ProjectPatch payload cannot be serialized: {error}"),
                            None,
                            "Regenerate the typed ProjectPatch payload.",
                        )
                    })?),
                    file_path: None,
                    expected_patch_id: Some(patch.patch_id.clone()),
                    dry_run: true,
                };
                let imported = ProjectPatchImportService::from_json_string(session, import_request);
                if !crate::project_patch_import_accepted(&imported) {
                    let codes = crate::import_diagnostics(&imported)
                        .into_iter()
                        .map(|diagnostic| diagnostic.code)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.project_patch_rejected",
                        format!("ProjectPatch payload failed lowering: {codes}"),
                        None,
                        "Fix ProjectPatch schema and validation diagnostics before retrying.",
                    ));
                }
                PreparedProjectCandidatePayload::ProjectPatch {
                    patch: imported
                        .parsed_patch
                        .expect("accepted import must contain parsed patch"),
                }
            }
            ProjectCandidatePayload::ControlledSourcePatch { request } => {
                validate_payload_root(&binding, &request.project_root)?;
                let candidate = ControlledSourcePatch::prepare(request.clone())
                    .map_err(controlled_source_patch_error)?;
                if candidate.revision.base_project_digest != binding.project_digest {
                    return Err(base_digest_mismatch_error());
                }
                PreparedProjectCandidatePayload::ControlledSourcePatch { candidate }
            }
            ProjectCandidatePayload::AssetImport {
                request,
                expected_source_hash,
            } => {
                validate_digest(expected_source_hash, "expected source hash")?;
                validate_payload_root(&binding, &request.project_root)?;
                let candidate = ProjectAssetImport::prepare(request.clone())
                    .map_err(project_asset_import_error)?;
                if candidate.source_hash != *expected_source_hash {
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.source_digest_mismatch",
                        "Asset source bytes do not match the envelope expected source hash.",
                        Some(Path::new(&candidate.source_path)),
                        "Recreate the candidate from the current source bytes.",
                    ));
                }
                if candidate.revision.base_project_digest != binding.project_digest {
                    return Err(base_digest_mismatch_error());
                }
                PreparedProjectCandidatePayload::AssetImport { candidate }
            }
        };
        let mut candidate = ProjectCandidate {
            schema_version: PROJECT_CANDIDATE_SCHEMA_VERSION.to_string(),
            envelope,
            project_binding: binding,
            source_digest,
            source_path,
            envelope_digest,
            payload_digest,
            candidate_digest: String::new(),
            prepared_payload,
            diagnostics: Vec::new(),
            next_actions: vec![
                "Validate this exact candidate before requesting explicit approval.".to_string(),
            ],
        };
        candidate.candidate_digest = candidate_digest(&candidate)?;
        validate_candidate_record(&candidate)?;
        Ok(candidate)
    }

    pub fn validate(
        session: &EditorSession,
        candidate: &ProjectCandidate,
        context: &ProjectCandidateValidationContext,
    ) -> Result<ProjectCandidateValidationReport, ProjectCandidateError> {
        validate_candidate_record(candidate)?;
        validate_source_binding(candidate)?;
        validate_live_candidate_binding(session, candidate)?;
        let (status, payload_validation, diagnostics) = match &candidate.prepared_payload {
            PreparedProjectCandidatePayload::ProjectPatch { patch } => {
                validate_project_patch_context(session, candidate)?;
                let report = PatchValidator::validate(session, patch);
                let diagnostics = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>();
                let status = if report.accepted {
                    ProjectCandidateValidationStatus::Passed
                } else {
                    ProjectCandidateValidationStatus::Failed
                };
                (
                    status,
                    ProjectCandidateValidationPayload::ProjectPatch { report },
                    diagnostics,
                )
            }
            PreparedProjectCandidatePayload::ControlledSourcePatch { candidate } => {
                let request = context.controlled_source_patch.as_ref().ok_or_else(|| {
                    ProjectCandidateError::new(
                        "project_candidate_entry.source_validation_context_required",
                        "ControlledSourcePatch validation requires an explicit engine SDK context.",
                        None,
                        "Provide compile-tests-only validation context for the trusted engine SDK.",
                    )
                })?;
                let report = ControlledSourcePatch::validate_cancellable(
                    candidate,
                    request,
                    context.cancellation.as_ref(),
                )
                .map_err(controlled_source_patch_error)?;
                let status =
                    if report.status == crate::ControlledSourcePatchValidationStatus::Passed {
                        ProjectCandidateValidationStatus::Passed
                    } else {
                        ProjectCandidateValidationStatus::Failed
                    };
                let diagnostics = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect();
                (
                    status,
                    ProjectCandidateValidationPayload::ControlledSourcePatch { report },
                    diagnostics,
                )
            }
            PreparedProjectCandidatePayload::AssetImport { candidate } => {
                let report =
                    ProjectAssetImport::validate(candidate).map_err(project_asset_import_error)?;
                let status = if report.status == crate::ProjectAssetImportValidationStatus::Passed {
                    ProjectCandidateValidationStatus::Passed
                } else {
                    ProjectCandidateValidationStatus::Failed
                };
                let diagnostics = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect();
                (
                    status,
                    ProjectCandidateValidationPayload::AssetImport { report },
                    diagnostics,
                )
            }
        };
        let mut report = ProjectCandidateValidationReport {
            schema_version: PROJECT_CANDIDATE_VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
            candidate_id: candidate.envelope.candidate_id.clone(),
            candidate_digest: candidate.candidate_digest.clone(),
            status,
            validation_digest: String::new(),
            payload_validation,
            diagnostics,
            next_actions: vec![
                "Approve the exact candidate and validation digest before apply.".to_string(),
            ],
        };
        report.validation_digest = validation_digest(&report)?;
        Ok(report)
    }

    pub fn apply(
        session: &mut EditorSession,
        candidate: ProjectCandidate,
        validation: ProjectCandidateValidationReport,
        approval: ProjectCandidateApproval,
    ) -> Result<ProjectCandidateApplyReceipt, ProjectCandidateError> {
        validate_candidate_record(&candidate)?;
        validate_validation_report(&candidate, &validation)?;
        validate_approval(&candidate, &validation, &approval)?;
        validate_source_binding(&candidate)?;
        validate_live_candidate_binding(session, &candidate)?;
        let before_project_digest = candidate.project_binding.project_digest.clone();
        let applied_payload = match (&candidate.prepared_payload, &validation.payload_validation) {
            (
                PreparedProjectCandidatePayload::ProjectPatch { patch },
                ProjectCandidateValidationPayload::ProjectPatch { report },
            ) => {
                validate_project_patch_context(session, &candidate)?;
                let current = PatchValidator::validate(session, patch);
                if &current != report {
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.validation_replay_mismatch",
                        "ProjectPatch validation evidence no longer matches the candidate.",
                        None,
                        "Validate and approve the candidate again against current state.",
                    ));
                }
                let apply_report = session.execute_patch_as_transaction(patch.clone());
                if apply_report.status != PatchApplyStatus::Committed {
                    let first_diagnostic = apply_report
                        .operation_results
                        .iter()
                        .flat_map(|result| {
                            result.diagnostics.iter().map(move |diagnostic| {
                                format!(
                                    "operation={} code={} message={}",
                                    result.operation_id, diagnostic.code, diagnostic.message
                                )
                            })
                        })
                        .next()
                        .or_else(|| {
                            apply_report
                                .validation
                                .diagnostics
                                .first()
                                .map(|diagnostic| {
                                    format!(
                                        "validation code={} message={}",
                                        diagnostic.code, diagnostic.message
                                    )
                                })
                        })
                        .unwrap_or_else(|| "no apply diagnostic was reported".to_string());
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.project_patch_apply_failed",
                        format!(
                            "ProjectPatch apply ended with {:?}: {first_diagnostic}",
                            apply_report.status
                        ),
                        None,
                        "Inspect ProjectPatch apply diagnostics and prepare a new candidate.",
                    ));
                }
                ProjectCandidateAppliedPayload::ProjectPatch {
                    patch_id: patch.patch_id.clone(),
                    report: apply_report,
                }
            }
            (
                PreparedProjectCandidatePayload::ControlledSourcePatch { candidate },
                ProjectCandidateValidationPayload::ControlledSourcePatch { report },
            ) => {
                let receipt = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                    candidate: candidate.clone(),
                    validation_report: report.clone(),
                    approval: ControlledSourcePatchApproval {
                        schema_version: crate::CONTROLLED_SOURCE_PATCH_APPROVAL_SCHEMA_VERSION
                            .to_string(),
                        patch_id: candidate.patch_id.clone(),
                        revision_id: candidate.revision.revision_id.clone(),
                        candidate_project_digest: candidate
                            .revision
                            .candidate_project_digest
                            .clone(),
                        validation_digest: report.validation_digest.clone(),
                        approved_by: approval.approved_by.clone(),
                    },
                })
                .map_err(controlled_source_patch_error)?;
                ProjectCandidateAppliedPayload::ControlledSourcePatch { receipt }
            }
            (
                PreparedProjectCandidatePayload::AssetImport { candidate },
                ProjectCandidateValidationPayload::AssetImport { report },
            ) => {
                let receipt = ProjectAssetImport::apply(ProjectAssetImportApplyRequest {
                    candidate: candidate.clone(),
                    validation_report: report.clone(),
                    approval: ProjectAssetImportApproval {
                        schema_version: crate::PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION
                            .to_string(),
                        approved_by: approval.approved_by.clone(),
                        candidate_digest: candidate.candidate_digest.clone(),
                        validation_digest: report.validation_digest.clone(),
                        allow_replace: approval.allow_replace,
                    },
                })
                .map_err(project_asset_import_error)?;
                ProjectCandidateAppliedPayload::AssetImport { receipt }
            }
            _ => return Err(validation_payload_mismatch_error()),
        };
        let applied_binding = Self::inspect_project_binding(session)?;
        if applied_binding.project_id != candidate.project_binding.project_id
            || applied_binding.project_root != candidate.project_binding.project_root
        {
            return Err(ProjectCandidateError::new(
                "project_candidate_entry.applied_project_binding_changed",
                "Candidate apply changed or escaped the bound project identity.",
                Some(Path::new(&applied_binding.project_root)),
                "Stop and recover the project from the delegated transaction receipt.",
            ));
        }
        let mut receipt = ProjectCandidateApplyReceipt {
            schema_version: PROJECT_CANDIDATE_APPLY_RECEIPT_SCHEMA_VERSION.to_string(),
            candidate_id: candidate.envelope.candidate_id,
            candidate_digest: candidate.candidate_digest,
            validation_digest: validation.validation_digest,
            project_id: applied_binding.project_id,
            project_root: applied_binding.project_root,
            before_project_digest,
            applied_project_digest: applied_binding.project_digest,
            applied_payload,
            receipt_binding_digest: String::new(),
            diagnostics: Vec::new(),
            next_actions: vec![
                "Keep this receipt until the candidate is accepted or rolled back.".to_string(),
            ],
        };
        receipt.receipt_binding_digest = apply_receipt_digest(&receipt)?;
        Ok(receipt)
    }

    pub fn rollback(
        session: &mut EditorSession,
        receipt: &ProjectCandidateApplyReceipt,
    ) -> Result<ProjectCandidateRollbackReceipt, ProjectCandidateError> {
        validate_apply_receipt(receipt)?;
        let current = Self::inspect_project_binding(session)?;
        if current.project_id != receipt.project_id
            || current.project_root != receipt.project_root
            || current.project_digest != receipt.applied_project_digest
        {
            return Err(ProjectCandidateError::new(
                "project_candidate_entry.rollback_project_drifted",
                "Project state no longer matches the candidate apply receipt.",
                Some(Path::new(&current.project_root)),
                "Do not replay rollback; review intervening project changes first.",
            ));
        }
        let rolled_back_payload = match &receipt.applied_payload {
            ProjectCandidateAppliedPayload::ProjectPatch { patch_id, .. } => {
                let report =
                    session
                        .rollback_last_project_patch(patch_id)
                        .map_err(|diagnostic| {
                            ProjectCandidateError::new(
                                diagnostic.code,
                                diagnostic.message,
                                diagnostic.target.as_deref().map(Path::new),
                                "Rollback only the exact latest ProjectPatch candidate apply.",
                            )
                        })?;
                if report.status != PatchApplyStatus::Committed {
                    let first_diagnostic = report
                        .operation_results
                        .iter()
                        .flat_map(|result| {
                            result.diagnostics.iter().map(move |diagnostic| {
                                format!(
                                    "operation={} code={} message={}",
                                    result.operation_id, diagnostic.code, diagnostic.message
                                )
                            })
                        })
                        .next()
                        .or_else(|| {
                            report.validation.diagnostics.first().map(|diagnostic| {
                                format!(
                                    "validation code={} message={}",
                                    diagnostic.code, diagnostic.message
                                )
                            })
                        })
                        .unwrap_or_else(|| "no rollback diagnostic was reported".to_string());
                    return Err(ProjectCandidateError::new(
                        "project_candidate_entry.project_patch_rollback_failed",
                        format!(
                            "ProjectPatch rollback ended with {:?}: {first_diagnostic}",
                            report.status
                        ),
                        None,
                        "Inspect inverse patch diagnostics and recover with a maintainer.",
                    ));
                }
                ProjectCandidateRolledBackPayload::ProjectPatch { report }
            }
            ProjectCandidateAppliedPayload::ControlledSourcePatch { receipt: delegated } => {
                let delegated_receipt =
                    ControlledSourcePatch::rollback(delegated, Path::new(&receipt.project_root))
                        .map_err(controlled_source_patch_error)?;
                ProjectCandidateRolledBackPayload::ControlledSourcePatch {
                    receipt: delegated_receipt,
                }
            }
            ProjectCandidateAppliedPayload::AssetImport { receipt: delegated } => {
                let delegated_receipt =
                    ProjectAssetImport::rollback(delegated, Path::new(&receipt.project_root))
                        .map_err(project_asset_import_error)?;
                ProjectCandidateRolledBackPayload::AssetImport {
                    receipt: delegated_receipt,
                }
            }
        };
        let restored = Self::inspect_project_binding(session)?;
        if restored.project_digest != receipt.before_project_digest {
            return Err(ProjectCandidateError::new(
                "project_candidate_entry.rollback_digest_mismatch",
                "Delegated rollback did not restore the bound pre-apply project digest.",
                Some(Path::new(&restored.project_root)),
                "Preserve rollback evidence and recover with a trusted maintainer.",
            ));
        }
        Ok(ProjectCandidateRollbackReceipt {
            schema_version: PROJECT_CANDIDATE_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
            candidate_id: receipt.candidate_id.clone(),
            restored_project_digest: restored.project_digest,
            replaced_project_digest: receipt.applied_project_digest.clone(),
            rolled_back_payload,
            diagnostics: Vec::new(),
            next_actions: vec!["The project is back at the candidate base revision.".to_string()],
        })
    }
}

fn validate_envelope(envelope: &ProjectCandidateEnvelope) -> Result<(), ProjectCandidateError> {
    if envelope.schema_version != PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.envelope_schema_unsupported",
            format!(
                "Unsupported candidate envelope schema: {}",
                envelope.schema_version
            ),
            None,
            "Regenerate the candidate using project-candidate-envelope.v1.",
        ));
    }
    validate_text_id(
        &envelope.candidate_id,
        "candidate id",
        MAX_CANDIDATE_ID_CHARS,
    )?;
    validate_text_id(
        &envelope.source_label,
        "source label",
        MAX_SOURCE_LABEL_CHARS,
    )?;
    validate_text_id(
        &envelope.target_project_id,
        "target project id",
        MAX_CANDIDATE_ID_CHARS,
    )?;
    validate_digest(
        &envelope.expected_base_project_digest,
        "base project digest",
    )?;
    match &envelope.payload {
        ProjectCandidatePayload::ProjectPatch(_) => {
            validate_digest(
                envelope
                    .project_patch_context_hash
                    .as_deref()
                    .unwrap_or_default(),
                "ProjectPatch context hash",
            )?;
        }
        _ if envelope.project_patch_context_hash.is_some() => {
            return Err(ProjectCandidateError::new(
                "project_candidate_entry.unexpected_context_hash",
                "Only ProjectPatch payloads may carry a ProjectPatch context hash.",
                None,
                "Remove projectPatchContextHash from this payload envelope.",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_project_binding(
    envelope: &ProjectCandidateEnvelope,
    binding: &ProjectCandidateProjectBinding,
) -> Result<(), ProjectCandidateError> {
    if envelope.target_project_id != binding.project_id {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.project_id_mismatch",
            "Candidate target project id does not match the active project.",
            Some(Path::new(&binding.project_root)),
            "Open the intended project or regenerate the candidate for this project.",
        ));
    }
    if envelope.expected_base_project_digest != binding.project_digest {
        return Err(base_digest_mismatch_error());
    }
    Ok(())
}

fn validate_live_candidate_binding(
    session: &EditorSession,
    candidate: &ProjectCandidate,
) -> Result<(), ProjectCandidateError> {
    let current = ProjectCandidateEntry::inspect_project_binding(session)?;
    if current.project_id != candidate.project_binding.project_id
        || current.project_root != candidate.project_binding.project_root
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.project_binding_mismatch",
            "Active project identity no longer matches the prepared candidate.",
            Some(Path::new(&current.project_root)),
            "Reopen the candidate project or prepare a new candidate.",
        ));
    }
    if current.project_digest != candidate.project_binding.project_digest {
        return Err(base_digest_mismatch_error());
    }
    Ok(())
}

fn validate_project_patch_context(
    session: &EditorSession,
    candidate: &ProjectCandidate,
) -> Result<(), ProjectCandidateError> {
    let current = ProjectPatchLlmContextSnapshot::capture(session).context_hash;
    if candidate.envelope.project_patch_context_hash.as_deref() != Some(current.as_str()) {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.context_drifted",
            "ProjectPatch semantic context changed after candidate preparation.",
            None,
            "Prepare and validate a new candidate against the current editor context.",
        ));
    }
    Ok(())
}

fn validate_payload_root(
    binding: &ProjectCandidateProjectBinding,
    payload_root: &Path,
) -> Result<(), ProjectCandidateError> {
    let canonical = payload_root.canonicalize().map_err(|error| {
        ProjectCandidateError::new(
            "project_candidate_entry.payload_project_root_invalid",
            format!("Payload project root cannot be canonicalized: {error}"),
            Some(payload_root),
            "Use the active project root in the payload prepare request.",
        )
    })?;
    if canonical != PathBuf::from(&binding.project_root) {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.payload_project_root_mismatch",
            "Payload prepare request does not target the active project root.",
            Some(&canonical),
            "Regenerate the payload request for the bound active project.",
        ));
    }
    Ok(())
}

fn validate_candidate_record(candidate: &ProjectCandidate) -> Result<(), ProjectCandidateError> {
    if candidate.schema_version != PROJECT_CANDIDATE_SCHEMA_VERSION {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.candidate_schema_unsupported",
            "Candidate record schema is unsupported.",
            None,
            "Prepare a new candidate with the current engine.",
        ));
    }
    validate_envelope(&candidate.envelope)?;
    if candidate.project_binding.schema_version != PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.binding_schema_unsupported",
            "Candidate project binding schema is unsupported.",
            Some(Path::new(&candidate.project_binding.project_root)),
            "Prepare a new candidate using the current engine.",
        ));
    }
    validate_project_binding(&candidate.envelope, &candidate.project_binding)?;
    validate_digest(&candidate.source_digest, "source digest")?;
    validate_digest(&candidate.envelope_digest, "envelope digest")?;
    validate_digest(&candidate.payload_digest, "payload digest")?;
    validate_digest(&candidate.candidate_digest, "candidate digest")?;
    if digest_serializable(&candidate.envelope, "candidate envelope")? != candidate.envelope_digest
        || digest_serializable(&candidate.envelope.payload, "candidate payload")?
            != candidate.payload_digest
        || candidate_digest(candidate)? != candidate.candidate_digest
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.candidate_tampered",
            "Candidate content no longer matches its canonical digest bindings.",
            None,
            "Discard the modified record and prepare a new candidate.",
        ));
    }
    let kinds_match = matches!(
        (&candidate.envelope.payload, &candidate.prepared_payload),
        (
            ProjectCandidatePayload::ProjectPatch(_),
            PreparedProjectCandidatePayload::ProjectPatch { .. }
        ) | (
            ProjectCandidatePayload::ControlledSourcePatch { .. },
            PreparedProjectCandidatePayload::ControlledSourcePatch { .. }
        ) | (
            ProjectCandidatePayload::AssetImport { .. },
            PreparedProjectCandidatePayload::AssetImport { .. }
        )
    );
    if !kinds_match {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.lowered_payload_mismatch",
            "Prepared payload kind does not match the envelope payload kind.",
            None,
            "Discard the invalid candidate and prepare it again.",
        ));
    }
    validate_lowering_binding(candidate)?;
    Ok(())
}

fn validate_lowering_binding(candidate: &ProjectCandidate) -> Result<(), ProjectCandidateError> {
    let valid = match (&candidate.envelope.payload, &candidate.prepared_payload) {
        (
            ProjectCandidatePayload::ProjectPatch(envelope_patch),
            PreparedProjectCandidatePayload::ProjectPatch { patch },
        ) => envelope_patch == patch,
        (
            ProjectCandidatePayload::ControlledSourcePatch { request },
            PreparedProjectCandidatePayload::ControlledSourcePatch {
                candidate: prepared,
            },
        ) => {
            request.revision_id == prepared.revision.revision_id
                && request.source_patch == prepared.source_patch
                && roots_match(
                    &request.project_root,
                    Path::new(&prepared.revision.project_root),
                )
                && roots_match(
                    &request.candidate_store_root,
                    Path::new(&prepared.candidate_store_root),
                )
        }
        (
            ProjectCandidatePayload::AssetImport {
                request,
                expected_source_hash,
            },
            PreparedProjectCandidatePayload::AssetImport {
                candidate: prepared,
            },
        ) => {
            let expected_descriptor = format!(
                "{}/{}.asset",
                request.target_directory.trim_end_matches(['/', '\\']),
                request.asset_id
            )
            .replace('\\', "/");
            request.import_id == prepared.import_id
                && request.revision_id == prepared.revision.revision_id
                && roots_match(
                    &request.project_root,
                    Path::new(&prepared.revision.project_root),
                )
                && roots_match(
                    &request.candidate_store_root,
                    Path::new(&prepared.candidate_store_root),
                )
                && roots_match(&request.source_path, Path::new(&prepared.source_path))
                && request.asset_id == prepared.record.asset_id
                && request.display_name == prepared.record.display_name
                && request.source_metadata == prepared.record.source_metadata
                && request.license == prepared.record.license
                && request.conflict_policy == prepared.conflict_policy
                && expected_source_hash == &prepared.source_hash
                && expected_descriptor == prepared.record.descriptor_path
                && digest_serializable(&request.texture_settings, "texture settings")?
                    == prepared.record.settings_hash
        }
        _ => false,
    };
    if !valid {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.lowering_binding_mismatch",
            "Prepared typed candidate no longer matches its envelope payload.",
            None,
            "Discard the record and lower a new candidate from the original envelope.",
        ));
    }
    Ok(())
}

fn roots_match(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn validate_source_binding(candidate: &ProjectCandidate) -> Result<(), ProjectCandidateError> {
    let Some(source_path) = candidate.source_path.as_deref() else {
        return Ok(());
    };
    let path = Path::new(source_path);
    let bytes = read_bounded_regular_file(path)?;
    let canonical = path.canonicalize().map_err(|error| {
        ProjectCandidateError::new(
            "project_candidate_entry.file_canonicalize_failed",
            format!("Candidate source file cannot be canonicalized: {error}"),
            Some(path),
            "Restore the original candidate source file or prepare a new candidate.",
        )
    })?;
    if canonical.display().to_string() != source_path
        || sha256_prefixed(&bytes) != candidate.source_digest
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.source_drifted",
            "Candidate source file changed after preparation.",
            Some(path),
            "Prepare and approve a new candidate from the current source file.",
        ));
    }
    Ok(())
}

fn validate_validation_report(
    candidate: &ProjectCandidate,
    report: &ProjectCandidateValidationReport,
) -> Result<(), ProjectCandidateError> {
    if report.schema_version != PROJECT_CANDIDATE_VALIDATION_REPORT_SCHEMA_VERSION
        || report.candidate_id != candidate.envelope.candidate_id
        || report.candidate_digest != candidate.candidate_digest
        || report.status != ProjectCandidateValidationStatus::Passed
        || validation_digest(report)? != report.validation_digest
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.validation_mismatch",
            "Validation report is not a passed exact binding for this candidate.",
            None,
            "Validate the exact candidate again before approval.",
        ));
    }
    Ok(())
}

fn validate_approval(
    candidate: &ProjectCandidate,
    validation: &ProjectCandidateValidationReport,
    approval: &ProjectCandidateApproval,
) -> Result<(), ProjectCandidateError> {
    if approval.schema_version != PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION
        || approval.candidate_id != candidate.envelope.candidate_id
        || approval.candidate_digest != candidate.candidate_digest
        || approval.validation_digest != validation.validation_digest
        || approval.approved_by.trim().is_empty()
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.approval_mismatch",
            "Approval does not bind the exact candidate and validation evidence.",
            None,
            "Issue a new explicit approval for this candidate and validation digest.",
        ));
    }
    Ok(())
}

fn validate_apply_receipt(
    receipt: &ProjectCandidateApplyReceipt,
) -> Result<(), ProjectCandidateError> {
    if receipt.schema_version != PROJECT_CANDIDATE_APPLY_RECEIPT_SCHEMA_VERSION
        || apply_receipt_digest(receipt)? != receipt.receipt_binding_digest
    {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.receipt_tampered",
            "Candidate apply receipt does not match its canonical binding digest.",
            None,
            "Preserve the project and recover using trusted transaction evidence.",
        ));
    }
    Ok(())
}

fn candidate_digest(candidate: &ProjectCandidate) -> Result<String, ProjectCandidateError> {
    let mut value = serde_json::to_value(candidate).map_err(serialization_error)?;
    value
        .as_object_mut()
        .expect("ProjectCandidate must serialize as an object")
        .insert(
            "candidateDigest".to_string(),
            serde_json::Value::String(String::new()),
        );
    digest_json_value(&value, "candidate")
}

fn validation_digest(
    report: &ProjectCandidateValidationReport,
) -> Result<String, ProjectCandidateError> {
    let mut value = serde_json::to_value(report).map_err(serialization_error)?;
    value
        .as_object_mut()
        .expect("ProjectCandidateValidationReport must serialize as an object")
        .insert(
            "validationDigest".to_string(),
            serde_json::Value::String(String::new()),
        );
    digest_json_value(&value, "candidate validation")
}

fn apply_receipt_digest(
    receipt: &ProjectCandidateApplyReceipt,
) -> Result<String, ProjectCandidateError> {
    let mut value = serde_json::to_value(receipt).map_err(serialization_error)?;
    value
        .as_object_mut()
        .expect("ProjectCandidateApplyReceipt must serialize as an object")
        .insert(
            "receiptBindingDigest".to_string(),
            serde_json::Value::String(String::new()),
        );
    digest_json_value(&value, "candidate apply receipt")
}

fn digest_serializable<T: Serialize>(
    value: &T,
    role: &str,
) -> Result<String, ProjectCandidateError> {
    let value = serde_json::to_value(value).map_err(serialization_error)?;
    digest_json_value(&value, role)
}

fn digest_json_value(
    value: &serde_json::Value,
    role: &str,
) -> Result<String, ProjectCandidateError> {
    canonical_json_bytes(value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.canonical_digest_failed",
                format!("Failed to digest {role}: {error}"),
                None,
                "Regenerate the candidate from canonical serializable data.",
            )
        })
}

fn serialization_error(error: serde_json::Error) -> ProjectCandidateError {
    ProjectCandidateError::new(
        "project_candidate_entry.serialization_failed",
        format!("Candidate evidence cannot be serialized: {error}"),
        None,
        "Regenerate the candidate using supported schema values.",
    )
}

fn read_bounded_regular_file(path: &Path) -> Result<Vec<u8>, ProjectCandidateError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ProjectCandidateError::new(
            "project_candidate_entry.file_inspection_failed",
            format!("Candidate file cannot be inspected: {error}"),
            Some(path),
            "Provide an existing regular candidate JSON file.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.file_not_regular",
            "Candidate path must be a regular file and cannot be a symlink.",
            Some(path),
            "Provide a regular UTF-8 JSON file.",
        ));
    }
    if metadata.len() > MAX_CANDIDATE_INPUT_BYTES as u64 {
        return Err(input_too_large_error());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(path)
        .and_then(|file| {
            file.take((MAX_CANDIDATE_INPUT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| {
            ProjectCandidateError::new(
                "project_candidate_entry.file_read_failed",
                format!("Candidate file cannot be read: {error}"),
                Some(path),
                "Fix file access and retry.",
            )
        })?;
    if bytes.len() > MAX_CANDIDATE_INPUT_BYTES {
        return Err(input_too_large_error());
    }
    Ok(bytes)
}

fn input_too_large_error() -> ProjectCandidateError {
    ProjectCandidateError::new(
        "project_candidate_entry.input_too_large",
        format!(
            "Candidate input exceeds the {} byte limit.",
            MAX_CANDIDATE_INPUT_BYTES
        ),
        None,
        "Split the work into smaller single-payload candidates.",
    )
}

fn validate_text_id(value: &str, role: &str, max: usize) -> Result<(), ProjectCandidateError> {
    let count = value.chars().count();
    if count == 0 || count > max || value.chars().any(|character| character.is_control()) {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.text_policy_failed",
            format!("{role} must contain 1..={max} non-control characters."),
            None,
            "Use a bounded printable identifier or label.",
        ));
    }
    Ok(())
}

fn validate_digest(value: &str, role: &str) -> Result<(), ProjectCandidateError> {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if !valid {
        return Err(ProjectCandidateError::new(
            "project_candidate_entry.digest_invalid",
            format!("{role} must be a sha256-prefixed digest."),
            None,
            "Regenerate the digest from canonical candidate evidence.",
        ));
    }
    Ok(())
}

fn import_source_kind(kind: ProjectCandidateSourceKind) -> ProjectPatchImportSourceKind {
    match kind {
        ProjectCandidateSourceKind::BuiltInProvider => {
            ProjectPatchImportSourceKind::AiStructuredOutput
        }
        ProjectCandidateSourceKind::ImportedFile => ProjectPatchImportSourceKind::FilePath,
        ProjectCandidateSourceKind::ImportedCodex => ProjectPatchImportSourceKind::JsonString,
        ProjectCandidateSourceKind::TestFixture => ProjectPatchImportSourceKind::TestFixture,
    }
}

fn base_digest_mismatch_error() -> ProjectCandidateError {
    ProjectCandidateError::new(
        "project_candidate_entry.base_project_drifted",
        "Candidate base project digest does not match the active project.",
        None,
        "Discard or regenerate the candidate from the current project state.",
    )
}

fn validation_payload_mismatch_error() -> ProjectCandidateError {
    ProjectCandidateError::new(
        "project_candidate_entry.validation_payload_mismatch",
        "Validation payload kind does not match the prepared candidate payload.",
        None,
        "Validate the exact candidate again before approval.",
    )
}

fn candidate_revision_error(error: crate::CandidateProjectRevisionError) -> ProjectCandidateError {
    ProjectCandidateError {
        code: error.code,
        message: error.message,
        path: error.path,
        next_action: error.next_action,
    }
}

fn controlled_source_patch_error(error: ControlledSourcePatchError) -> ProjectCandidateError {
    ProjectCandidateError {
        code: error.code,
        message: error.message,
        path: error.path,
        next_action: error.next_action,
    }
}

fn project_asset_import_error(error: ProjectAssetImportError) -> ProjectCandidateError {
    ProjectCandidateError {
        code: error.code,
        message: error.message,
        path: error.path,
        next_action: error.next_action,
    }
}
