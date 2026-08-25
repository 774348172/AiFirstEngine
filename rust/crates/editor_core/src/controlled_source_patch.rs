use crate::{
    CandidateBaseVerificationStatus, CandidateFileChange, CandidateProjectRevision,
    CandidateProjectRevisionRequest, CandidateProjectRevisionStore, ProjectManifest,
    ProjectRelativePath, ProjectRuntimeSourceKind, ProjectWriteScope,
    CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION,
    PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use runtime_cli::{
    run_bounded_child_process, run_bounded_child_process_cancellable,
    BoundedChildProcessCancellation, BoundedChildProcessExitReason, BoundedChildProcessRequest,
    BoundedChildProcessResult,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION: &str = "controlled-source-patch.v1";
pub const CONTROLLED_SOURCE_PATCH_CANDIDATE_SCHEMA_VERSION: &str =
    "controlled-source-patch-candidate.v1";
pub const CONTROLLED_SOURCE_PATCH_VALIDATION_REPORT_SCHEMA_VERSION: &str =
    "controlled-source-patch-validation-report.v1";
pub const CONTROLLED_SOURCE_PATCH_APPROVAL_SCHEMA_VERSION: &str =
    "controlled-source-patch-approval.v1";
pub const CONTROLLED_SOURCE_PATCH_APPLY_RECEIPT_SCHEMA_VERSION: &str =
    "controlled-source-patch-apply-receipt.v1";
pub const CONTROLLED_SOURCE_PATCH_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str =
    "controlled-source-patch-rollback-receipt.v1";

const ROLLBACK_RECORD_SCHEMA_VERSION: &str = "controlled-source-patch-rollback-record.v1";
const MAX_OPERATIONS: usize = 64;
const MAX_FILE_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_TEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ROLLBACK_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_STEP_TIMEOUT_MS: u64 = 120_000;
const MAX_STEP_TIMEOUT_MS: u64 = 600_000;
const DEFAULT_CAPTURE_LIMIT_BYTES: usize = 128 * 1024;
const MAX_CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;
const PROJECT_MANIFEST_PATH: &str = "project.aife.json";
const CARGO_MANIFEST_PATH: &str = "RuntimeModule/Cargo.toml";
const RUNTIME_LIB_PATH: &str = "RuntimeModule/src/lib.rs";
static VALIDATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchDocument {
    pub schema_version: String,
    pub patch_id: String,
    pub operations: Vec<ControlledSourcePatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlledSourcePatchOperation {
    CreateOrReplace { path: String, text: String },
    Delete { path: String },
}

impl ControlledSourcePatchOperation {
    fn path(&self) -> &str {
        match self {
            Self::CreateOrReplace { path, .. } | Self::Delete { path } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchPrepareRequest {
    pub revision_id: String,
    pub project_root: PathBuf,
    pub candidate_store_root: PathBuf,
    pub source_patch: ControlledSourcePatchDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchCandidate {
    pub schema_version: String,
    pub patch_id: String,
    pub patch_digest: String,
    pub source_patch: ControlledSourcePatchDocument,
    pub candidate_store_root: String,
    pub requested_paths: Vec<String>,
    pub revision: CandidateProjectRevision,
    pub diagnostics: Vec<ControlledSourcePatchDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlledSourcePatchDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchDiagnostic {
    pub severity: ControlledSourcePatchDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: Option<String>,
}

impl ControlledSourcePatchDiagnostic {
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ControlledSourcePatchDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
            next_action: None,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedEngineSdkLocator {
    pub sdk_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlledSourcePatchExecutionPolicy {
    CompileTestsOnly,
    TrustedLocalExecuteTests,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchValidationRequest {
    pub engine_sdk: TrustedEngineSdkLocator,
    pub execution_policy: ControlledSourcePatchExecutionPolicy,
    pub cargo_executable: Option<PathBuf>,
    pub step_timeout_ms: u64,
    pub capture_limit_bytes: usize,
}

impl ControlledSourcePatchValidationRequest {
    pub fn compile_tests_only(engine_sdk_root: impl Into<PathBuf>) -> Self {
        Self {
            engine_sdk: TrustedEngineSdkLocator {
                sdk_root: engine_sdk_root.into(),
            },
            execution_policy: ControlledSourcePatchExecutionPolicy::CompileTestsOnly,
            cargo_executable: None,
            step_timeout_ms: DEFAULT_STEP_TIMEOUT_MS,
            capture_limit_bytes: DEFAULT_CAPTURE_LIMIT_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlledSourcePatchValidationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlledSourcePatchValidationStepStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchValidationStep {
    pub name: String,
    pub status: ControlledSourcePatchValidationStepStatus,
    pub command: Vec<String>,
    pub process: BoundedChildProcessResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchValidationReport {
    pub schema_version: String,
    pub status: ControlledSourcePatchValidationStatus,
    pub patch_id: String,
    pub patch_digest: String,
    pub revision_id: String,
    pub base_project_digest: String,
    pub candidate_project_digest: String,
    pub changed_paths: Vec<String>,
    pub validation_digest: String,
    pub execution_policy: ControlledSourcePatchExecutionPolicy,
    pub engine_sdk_root: String,
    pub validation_root: String,
    pub cleanup_status: String,
    pub isolation_notice: String,
    pub steps: Vec<ControlledSourcePatchValidationStep>,
    pub diagnostics: Vec<ControlledSourcePatchDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchApproval {
    pub schema_version: String,
    pub patch_id: String,
    pub revision_id: String,
    pub candidate_project_digest: String,
    pub validation_digest: String,
    pub approved_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchApplyRequest {
    pub candidate: ControlledSourcePatchCandidate,
    pub validation_report: ControlledSourcePatchValidationReport,
    pub approval: ControlledSourcePatchApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchApplyReceipt {
    pub schema_version: String,
    pub patch_id: String,
    pub revision: CandidateProjectRevision,
    pub validation_digest: String,
    pub before_project_digest: String,
    pub applied_project_digest: String,
    pub changed_paths: Vec<String>,
    pub rollback_record_path: String,
    pub rollback_record_digest: String,
    pub receipt_binding_digest: String,
    pub diagnostics: Vec<ControlledSourcePatchDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchRollbackReceipt {
    pub schema_version: String,
    pub patch_id: String,
    pub revision_id: String,
    pub restored_project_digest: String,
    pub replaced_project_digest: String,
    pub changed_paths: Vec<String>,
    pub rollback_record_removed: bool,
    pub diagnostics: Vec<ControlledSourcePatchDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlledSourcePatchError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

impl ControlledSourcePatchError {
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

impl std::fmt::Display for ControlledSourcePatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ControlledSourcePatchError {}

pub struct ControlledSourcePatch;

impl ControlledSourcePatch {
    pub fn prepare(
        request: ControlledSourcePatchPrepareRequest,
    ) -> Result<ControlledSourcePatchCandidate, ControlledSourcePatchError> {
        let source_patch = request.source_patch;
        let requested_paths = validate_source_patch(&source_patch)?;
        let patch_digest = digest_serializable(&source_patch, "source patch")?;
        let patch_id = source_patch.patch_id.clone();
        let changes = lower_operations(source_patch.operations.clone());
        let revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: request.revision_id,
            project_root: request.project_root,
            candidate_store_root: request.candidate_store_root,
            changes,
        })
        .map_err(candidate_error)?;
        let candidate_store_root = Path::new(&revision.candidate_root)
            .parent()
            .ok_or_else(|| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.candidate_store_missing",
                    "Staged candidate has no owning store directory.",
                    Some(Path::new(&revision.candidate_root)),
                    "Discard the invalid candidate and retry.",
                )
            })?
            .to_path_buf();
        let candidate = ControlledSourcePatchCandidate {
            schema_version: CONTROLLED_SOURCE_PATCH_CANDIDATE_SCHEMA_VERSION.to_string(),
            patch_id,
            patch_digest,
            source_patch,
            candidate_store_root: candidate_store_root.display().to_string(),
            requested_paths,
            revision,
            diagnostics: Vec::new(),
            next_actions: vec![
                "Validate the isolated candidate before requesting approval.".to_string(),
            ],
        };

        if let Err(error) = validate_candidate_record(&candidate) {
            let _ = CandidateProjectRevisionStore::discard(
                &candidate.revision,
                Path::new(&candidate.candidate_store_root),
            );
            return Err(error);
        }
        if candidate.revision.changed_paths.is_empty() {
            let _ = CandidateProjectRevisionStore::discard(
                &candidate.revision,
                Path::new(&candidate.candidate_store_root),
            );
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.no_effect",
                "SourcePatch does not change the staged project bytes.",
                None,
                "Remove no-op operations or regenerate the patch from the current project.",
            ));
        }
        if let Err(error) =
            validate_candidate_contract(Path::new(&candidate.revision.candidate_root))
        {
            let _ = CandidateProjectRevisionStore::discard(
                &candidate.revision,
                Path::new(&candidate.candidate_store_root),
            );
            return Err(error);
        }
        Ok(candidate)
    }

    pub fn validate(
        candidate: &ControlledSourcePatchCandidate,
        request: &ControlledSourcePatchValidationRequest,
    ) -> Result<ControlledSourcePatchValidationReport, ControlledSourcePatchError> {
        Self::validate_cancellable(candidate, request, None)
    }

    pub fn validate_cancellable(
        candidate: &ControlledSourcePatchCandidate,
        request: &ControlledSourcePatchValidationRequest,
        cancellation: Option<&BoundedChildProcessCancellation>,
    ) -> Result<ControlledSourcePatchValidationReport, ControlledSourcePatchError> {
        validate_candidate_record(candidate)?;
        let project_root = Path::new(&candidate.revision.project_root);
        let base = CandidateProjectRevisionStore::verify_base(&candidate.revision, project_root)
            .map_err(candidate_error)?;
        if base.status != CandidateBaseVerificationStatus::Matched {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.base_drifted",
                "Project content changed after the candidate was prepared.",
                Some(project_root),
                "Discard or rebase the candidate before validation.",
            ));
        }
        let cargo_contract =
            validate_candidate_contract(Path::new(&candidate.revision.candidate_root))?;
        let sdk = resolve_engine_sdk(&request.engine_sdk, &cargo_contract)?;
        let store_root = canonical_store_root(candidate)?;
        let validation_root = validation_root(&store_root, &candidate.revision.revision_id);
        fs::create_dir(&validation_root).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.validation_root_create_failed",
                format!("Validation root cannot be created: {error}"),
                Some(&validation_root),
                "Check candidate store permissions and retry.",
            )
        })?;

        let validation_result = (|| {
            let runtime_module_root = validation_root.join("RuntimeModule");
            copy_runtime_module(
                &Path::new(&candidate.revision.candidate_root).join("RuntimeModule"),
                &runtime_module_root,
            )?;
            write_engine_sdk_patch_config(&runtime_module_root, &sdk)?;
            Ok(run_cargo_validation(
                candidate,
                request,
                &sdk,
                &validation_root,
                &runtime_module_root,
                cancellation,
            ))
        })();

        let cleanup_error = fs::remove_dir_all(&validation_root).err();
        let mut report = match validation_result {
            Ok(report) => report,
            Err(error) => {
                if let Some(cleanup_error) = cleanup_error {
                    return Err(ControlledSourcePatchError::new(
                        "controlled_source_patch.validation_setup_and_cleanup_failed",
                        format!("{error}; validation cleanup also failed: {cleanup_error}"),
                        Some(&validation_root),
                        "Close processes using the validation root and remove it manually.",
                    ));
                }
                return Err(error);
            }
        };
        if let Some(error) = cleanup_error {
            report.status = ControlledSourcePatchValidationStatus::Failed;
            report.cleanup_status = "failed".to_string();
            report.diagnostics.push(
                ControlledSourcePatchDiagnostic::error(
                    "controlled_source_patch.validation_cleanup_failed",
                    format!("Validation root cleanup failed: {error}"),
                )
                .with_path(validation_root.display().to_string())
                .with_next_action("Close processes using the validation root and remove it."),
            );
        } else {
            report.cleanup_status = "removed".to_string();
        }
        report.validation_digest = validation_report_digest(&report)?;
        Ok(report)
    }

    pub fn apply(
        request: ControlledSourcePatchApplyRequest,
    ) -> Result<ControlledSourcePatchApplyReceipt, ControlledSourcePatchError> {
        validate_candidate_record(&request.candidate)?;
        validate_passed_report(&request.candidate, &request.validation_report)?;
        validate_approval(
            &request.candidate,
            &request.validation_report,
            &request.approval,
        )?;
        let project_root = Path::new(&request.candidate.revision.project_root);
        let base =
            CandidateProjectRevisionStore::verify_base(&request.candidate.revision, project_root)
                .map_err(candidate_error)?;
        if base.status != CandidateBaseVerificationStatus::Matched {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.apply_base_drifted",
                "Project content changed after validation and approval.",
                Some(project_root),
                "Create and approve a new candidate from the current project.",
            ));
        }
        validate_candidate_contract(Path::new(&request.candidate.revision.candidate_root))?;

        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        let snapshots = snapshot_before_files(&scope, &request.candidate.revision.changed_paths)?;
        let store_root = canonical_store_root(&request.candidate)?;
        let rollback_name = rollback_record_name(&request.candidate.revision.revision_id);
        let rollback_path = store_root.join(&rollback_name);
        if rollback_path.exists() {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_record_exists",
                "A rollback record already exists for this revision.",
                Some(&rollback_path),
                "Resolve the previous apply or rollback before retrying.",
            ));
        }
        let mut rollback_record = RollbackRecord {
            schema_version: ROLLBACK_RECORD_SCHEMA_VERSION.to_string(),
            patch_id: request.candidate.patch_id.clone(),
            revision_id: request.candidate.revision.revision_id.clone(),
            before_project_digest: request.candidate.revision.base_project_digest.clone(),
            applied_project_digest: request.candidate.revision.candidate_project_digest.clone(),
            changed_paths: request.candidate.revision.changed_paths.clone(),
            record_digest: String::new(),
            receipt_binding_digest: String::new(),
            snapshots,
        };
        rollback_record.record_digest = rollback_record_digest(&rollback_record)?;
        let binding = receipt_binding_digest(
            &request.candidate.patch_id,
            &request.candidate.revision,
            &request.validation_report.validation_digest,
            &rollback_path,
            &rollback_record.record_digest,
        )?;
        rollback_record.receipt_binding_digest = binding.clone();
        let record_bytes = serde_json::to_vec(&rollback_record).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_record_encode_failed",
                format!("Rollback record cannot be encoded: {error}"),
                Some(&rollback_path),
                "Inspect the rollback record schema implementation.",
            )
        })?;
        ProjectWriteScope::open(&store_root)
            .map_err(project_write_error)?
            .write_atomic(&rollback_name, &record_bytes)
            .map_err(project_write_error)?;

        if let Err(error) = apply_candidate_files(&scope, &request.candidate.revision) {
            return fail_apply_and_restore(
                error,
                &scope,
                &rollback_record,
                &store_root,
                &rollback_name,
                &request.candidate.revision,
            );
        }
        let applied = match CandidateProjectRevisionStore::verify_base(
            &request.candidate.revision,
            project_root,
        )
        .map_err(candidate_error)
        {
            Ok(applied) => applied,
            Err(error) => {
                return fail_apply_and_restore(
                    error,
                    &scope,
                    &rollback_record,
                    &store_root,
                    &rollback_name,
                    &request.candidate.revision,
                );
            }
        };
        if applied.actual_digest != request.candidate.revision.candidate_project_digest {
            return fail_apply_and_restore(
                ControlledSourcePatchError::new(
                    "controlled_source_patch.apply_digest_mismatch",
                    "Applied project digest does not match the validated candidate.",
                    Some(project_root),
                    "Restore the before snapshot and inspect concurrent project writes.",
                ),
                &scope,
                &rollback_record,
                &store_root,
                &rollback_name,
                &request.candidate.revision,
            );
        }

        Ok(ControlledSourcePatchApplyReceipt {
            schema_version: CONTROLLED_SOURCE_PATCH_APPLY_RECEIPT_SCHEMA_VERSION.to_string(),
            patch_id: request.candidate.patch_id,
            revision: request.candidate.revision,
            validation_digest: request.validation_report.validation_digest,
            before_project_digest: rollback_record.before_project_digest,
            applied_project_digest: rollback_record.applied_project_digest,
            changed_paths: rollback_record.changed_paths,
            rollback_record_path: rollback_path.display().to_string(),
            rollback_record_digest: rollback_record.record_digest,
            receipt_binding_digest: binding,
            diagnostics: Vec::new(),
            next_actions: vec![
                "Keep the candidate and rollback record until this apply is accepted.".to_string(),
            ],
        })
    }

    pub fn rollback(
        receipt: &ControlledSourcePatchApplyReceipt,
        project_root: &Path,
    ) -> Result<ControlledSourcePatchRollbackReceipt, ControlledSourcePatchError> {
        validate_apply_receipt(receipt, project_root)?;
        let current = CandidateProjectRevisionStore::verify_base(&receipt.revision, project_root)
            .map_err(candidate_error)?;
        if current.actual_digest != receipt.applied_project_digest {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_project_drifted",
                "Project content changed after SourcePatch apply.",
                Some(project_root),
                "Review current changes and perform an explicit merge or recovery.",
            ));
        }
        let rollback_path = PathBuf::from(&receipt.rollback_record_path);
        let store_root = rollback_path.parent().ok_or_else(|| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_record_path_invalid",
                "Rollback record has no owning store directory.",
                Some(&rollback_path),
                "Reject the invalid apply receipt.",
            )
        })?;
        let rollback_name = rollback_record_name(&receipt.revision.revision_id);
        if rollback_path.file_name().and_then(|value| value.to_str()) != Some(&rollback_name) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_record_path_invalid",
                "Rollback record path is not the exact revision sibling.",
                Some(&rollback_path),
                "Reject the invalid apply receipt.",
            ));
        }
        let store_scope = ProjectWriteScope::open(store_root).map_err(project_write_error)?;
        let record_bytes = store_scope
            .read(&rollback_name)
            .map_err(project_write_error)?;
        let record: RollbackRecord = serde_json::from_slice(&record_bytes).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_record_invalid",
                format!("Rollback record cannot be decoded: {error}"),
                Some(&rollback_path),
                "Preserve the record and recover it with a trusted maintainer.",
            )
        })?;
        validate_rollback_record(receipt, &record)?;

        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        restore_snapshots(&scope, &record.snapshots)?;
        let restored = CandidateProjectRevisionStore::verify_base(&receipt.revision, project_root)
            .map_err(candidate_error)?;
        if restored.actual_digest != receipt.before_project_digest {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.rollback_digest_mismatch",
                "Rollback did not restore the recorded before digest.",
                Some(project_root),
                "Preserve the rollback record and recover with a trusted maintainer.",
            ));
        }
        store_scope
            .remove_file(&rollback_name)
            .map_err(project_write_error)?;
        Ok(ControlledSourcePatchRollbackReceipt {
            schema_version: CONTROLLED_SOURCE_PATCH_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
            patch_id: receipt.patch_id.clone(),
            revision_id: receipt.revision.revision_id.clone(),
            restored_project_digest: receipt.before_project_digest.clone(),
            replaced_project_digest: receipt.applied_project_digest.clone(),
            changed_paths: receipt.changed_paths.clone(),
            rollback_record_removed: true,
            diagnostics: Vec::new(),
            next_actions: vec!["The project is back at the pre-apply revision.".to_string()],
        })
    }
}

#[derive(Debug)]
struct CargoContract {
    engine_version: String,
    engine_dependencies: BTreeMap<String, String>,
}

#[derive(Debug)]
struct ResolvedEngineSdk {
    root: PathBuf,
    engine_runtime_root: PathBuf,
    engine_input_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RollbackFileSnapshot {
    path: String,
    before_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RollbackRecord {
    schema_version: String,
    patch_id: String,
    revision_id: String,
    before_project_digest: String,
    applied_project_digest: String,
    changed_paths: Vec<String>,
    record_digest: String,
    receipt_binding_digest: String,
    snapshots: Vec<RollbackFileSnapshot>,
}

fn validate_source_patch(
    patch: &ControlledSourcePatchDocument,
) -> Result<Vec<String>, ControlledSourcePatchError> {
    if patch.schema_version != CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.schema_unsupported",
            format!("Unsupported SourcePatch schema: {}", patch.schema_version),
            None,
            "Regenerate the patch using controlled-source-patch.v1.",
        ));
    }
    validate_opaque_id(&patch.patch_id, "patch id")?;
    if patch.operations.is_empty() || patch.operations.len() > MAX_OPERATIONS {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.operation_count_invalid",
            format!("SourcePatch must contain 1-{MAX_OPERATIONS} operations."),
            None,
            "Split or regenerate the patch within the operation limit.",
        ));
    }
    let mut paths = Vec::with_capacity(patch.operations.len());
    let mut comparable_paths = BTreeSet::new();
    let mut total_text_bytes = 0_usize;
    let mut previous: Option<&str> = None;
    for operation in &patch.operations {
        let path = operation.path();
        let relative = ProjectRelativePath::parse(path).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.path_invalid",
                format!("SourcePatch path is invalid: {error}"),
                Some(Path::new(path)),
                "Use a canonical project-relative allowlist path.",
            )
        })?;
        if relative.as_str() != path || !is_source_patch_path_allowed(path) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.path_not_allowed",
                "SourcePatch path is outside the project-owned RuntimeModule allowlist.",
                Some(Path::new(path)),
                "Target project.aife.json, RuntimeModule/Cargo.toml, or Rust source/test files.",
            ));
        }
        validate_portable_path(&relative)?;
        if previous.is_some_and(|value| value >= path) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.operations_not_sorted_unique",
                "SourcePatch operations must be uniquely sorted by canonical path.",
                Some(Path::new(path)),
                "Sort operations lexicographically and remove duplicate paths.",
            ));
        }
        let comparable = path.to_ascii_lowercase();
        if !comparable_paths.insert(comparable) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.path_case_collision",
                "SourcePatch paths collide on a case-insensitive filesystem.",
                Some(Path::new(path)),
                "Use one canonical spelling for each project path.",
            ));
        }
        if let ControlledSourcePatchOperation::CreateOrReplace { text, .. } = operation {
            let bytes = text.len();
            if bytes > MAX_FILE_TEXT_BYTES {
                return Err(ControlledSourcePatchError::new(
                    "controlled_source_patch.file_too_large",
                    format!("SourcePatch file text exceeds {MAX_FILE_TEXT_BYTES} bytes."),
                    Some(Path::new(path)),
                    "Split the implementation into smaller project-owned Rust files.",
                ));
            }
            total_text_bytes = total_text_bytes.checked_add(bytes).ok_or_else(|| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.total_text_too_large",
                    "SourcePatch total text size overflowed the supported range.",
                    None,
                    "Split the patch into smaller revisions.",
                )
            })?;
            if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
                return Err(ControlledSourcePatchError::new(
                    "controlled_source_patch.total_text_too_large",
                    format!("SourcePatch text exceeds {MAX_TOTAL_TEXT_BYTES} total bytes."),
                    None,
                    "Split the patch into smaller revisions.",
                ));
            }
        } else if matches!(
            path,
            PROJECT_MANIFEST_PATH | CARGO_MANIFEST_PATH | RUNTIME_LIB_PATH
        ) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.required_file_delete_rejected",
                "SourcePatch cannot delete a required ProjectRust contract file.",
                Some(Path::new(path)),
                "Replace the required file with valid content instead.",
            ));
        }
        previous = Some(path);
        paths.push(path.to_string());
    }
    Ok(paths)
}

fn lower_operations(operations: Vec<ControlledSourcePatchOperation>) -> Vec<CandidateFileChange> {
    operations
        .into_iter()
        .map(|operation| match operation {
            ControlledSourcePatchOperation::CreateOrReplace { path, text } => {
                CandidateFileChange::CreateOrReplace {
                    path,
                    bytes: text.into_bytes(),
                }
            }
            ControlledSourcePatchOperation::Delete { path } => CandidateFileChange::Delete { path },
        })
        .collect()
}

fn validate_candidate_record(
    candidate: &ControlledSourcePatchCandidate,
) -> Result<(), ControlledSourcePatchError> {
    if candidate.schema_version != CONTROLLED_SOURCE_PATCH_CANDIDATE_SCHEMA_VERSION {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.candidate_schema_unsupported",
            "Controlled SourcePatch candidate schema is unsupported.",
            None,
            "Discard and recreate the candidate.",
        ));
    }
    validate_opaque_id(&candidate.patch_id, "patch id")?;
    validate_digest(&candidate.patch_digest, "patch digest")?;
    let requested_paths = validate_source_patch(&candidate.source_patch)?;
    if candidate.source_patch.patch_id != candidate.patch_id
        || digest_serializable(&candidate.source_patch, "source patch")? != candidate.patch_digest
        || requested_paths != candidate.requested_paths
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.patch_binding_mismatch",
            "Candidate does not bind its exact structured SourcePatch document.",
            None,
            "Discard and recreate the candidate through prepare.",
        ));
    }
    if candidate.revision.schema_version != CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.revision_schema_unsupported",
            "CandidateProjectRevision schema is unsupported.",
            None,
            "Discard and recreate the candidate.",
        ));
    }
    let store = canonical_store_root(candidate)?;
    let candidate_root = Path::new(&candidate.revision.candidate_root)
        .canonicalize()
        .map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.candidate_root_unavailable",
                format!("Candidate root is unavailable: {error}"),
                Some(Path::new(&candidate.revision.candidate_root)),
                "Discard and recreate the candidate.",
            )
        })?;
    if candidate_root.parent() != Some(store.as_path())
        || candidate_root.file_name().and_then(|value| value.to_str())
            != Some(candidate.revision.revision_id.as_str())
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.candidate_store_mismatch",
            "Candidate root is not the exact revision child of its recorded store.",
            Some(&candidate_root),
            "Reject the candidate and inspect its provenance.",
        ));
    }
    if candidate.requested_paths.is_empty()
        || candidate.requested_paths.len() > MAX_OPERATIONS
        || !is_sorted_unique(&candidate.requested_paths)
        || candidate
            .requested_paths
            .iter()
            .any(|path| !is_source_patch_path_allowed(path))
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.requested_paths_invalid",
            "Candidate requested paths are not canonical SourcePatch allowlist paths.",
            None,
            "Discard and recreate the candidate through prepare.",
        ));
    }
    if candidate.revision.changed_paths.iter().any(|path| {
        !candidate.requested_paths.contains(path) || !is_source_patch_path_allowed(path)
    }) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.changed_paths_outside_request",
            "Candidate diff contains a path that was not requested by SourcePatch.",
            None,
            "Discard and recreate the candidate through prepare.",
        ));
    }
    Ok(())
}

fn validate_candidate_contract(
    candidate_root: &Path,
) -> Result<CargoContract, ControlledSourcePatchError> {
    let manifest_text = read_regular_utf8(&candidate_root.join(PROJECT_MANIFEST_PATH))?;
    let manifest_value: serde_json::Value =
        serde_json::from_str(&manifest_text).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.project_manifest_invalid",
                format!("Project manifest is invalid JSON: {error}"),
                Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
                "Write a valid aife-project.v2 ProjectRust manifest.",
            )
        })?;
    validate_project_manifest_keys(&manifest_value, candidate_root)?;
    let manifest: ProjectManifest = serde_json::from_value(manifest_value).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.project_manifest_invalid",
            format!("Project manifest does not match its schema: {error}"),
            Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
            "Write a valid aife-project.v2 ProjectRust manifest.",
        )
    })?;
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION
        || manifest.runtime_module.source_kind != Some(ProjectRuntimeSourceKind::ProjectRust)
        || manifest.runtime_module.cargo_manifest != CARGO_MANIFEST_PATH
        || manifest.runtime_module.interface_version != PROJECT_RUNTIME_MODULE_INTERFACE_VERSION
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.project_rust_contract_required",
            "Candidate manifest must explicitly select ProjectRust and RuntimeModule/Cargo.toml.",
            Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
            "Update runtimeModule to the controlled ProjectRust contract.",
        ));
    }
    manifest.runtime_module.validate().map_err(|code| {
        ControlledSourcePatchError::new(
            code,
            "Candidate runtimeModule contract is invalid.",
            Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
            "Fix the ProjectRust runtimeModule fields.",
        )
    })?;
    validate_cargo_package_name(&manifest.runtime_module.cargo_package)?;
    ensure_regular_file(&candidate_root.join(RUNTIME_LIB_PATH))?;
    if candidate_root.join("RuntimeModule/build.rs").exists() {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.build_script_rejected",
            "RuntimeModule/build.rs is forbidden by SourcePatch v1.",
            Some(&candidate_root.join("RuntimeModule/build.rs")),
            "Remove the build script from the project RuntimeModule.",
        ));
    }
    if candidate_root.join("RuntimeModule/.cargo").exists() {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.cargo_config_rejected",
            "Project-owned RuntimeModule Cargo config is forbidden during controlled validation.",
            Some(&candidate_root.join("RuntimeModule/.cargo")),
            "Remove project-owned Cargo config and use the trusted Engine SDK locator.",
        ));
    }
    validate_cargo_manifest(
        &candidate_root.join(CARGO_MANIFEST_PATH),
        &manifest.runtime_module.cargo_package,
        &manifest.engine_version,
    )
}

fn validate_project_manifest_keys(
    value: &serde_json::Value,
    candidate_root: &Path,
) -> Result<(), ControlledSourcePatchError> {
    const ALLOWED: &[&str] = &[
        "schemaVersion",
        "projectId",
        "projectName",
        "engineVersion",
        "createdAt",
        "lastOpenedAt",
        "defaultScene",
        "assetRoot",
        "settingsVersion",
        "runtimeModule",
    ];
    let object = value.as_object().ok_or_else(|| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.project_manifest_invalid",
            "Project manifest root must be an object.",
            Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
            "Write a valid aife-project.v2 manifest object.",
        )
    })?;
    if let Some(key) = object.keys().find(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.project_manifest_unknown_field",
            format!("Project manifest contains unsupported field: {key}"),
            Some(&candidate_root.join(PROJECT_MANIFEST_PATH)),
            "Remove unsupported manifest fields from this v1 SourcePatch.",
        ));
    }
    Ok(())
}

fn validate_cargo_manifest(
    path: &Path,
    expected_package: &str,
    engine_version: &str,
) -> Result<CargoContract, ControlledSourcePatchError> {
    let text = read_regular_utf8(path)?;
    let value: toml::Value = toml::from_str(&text).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.cargo_manifest_invalid",
            format!("RuntimeModule Cargo manifest is invalid TOML: {error}"),
            Some(path),
            "Write a valid restricted RuntimeModule Cargo manifest.",
        )
    })?;
    let root = value.as_table().ok_or_else(|| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.cargo_manifest_invalid",
            "RuntimeModule Cargo manifest root must be a TOML table.",
            Some(path),
            "Write a valid restricted RuntimeModule Cargo manifest.",
        )
    })?;
    for forbidden in [
        "workspace",
        "patch",
        "replace",
        "target",
        "build-dependencies",
        "dev-dependencies",
        "features",
        "bin",
        "example",
        "test",
        "bench",
    ] {
        if root.contains_key(forbidden) {
            return Err(cargo_policy_error(
                path,
                format!("Cargo manifest field/table '{forbidden}' is forbidden."),
            ));
        }
    }
    if contains_workspace_inheritance(&value) {
        return Err(cargo_policy_error(
            path,
            "Cargo workspace inheritance is forbidden.".to_string(),
        ));
    }
    let package = root
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            cargo_policy_error(path, "Cargo [package] table is required.".to_string())
        })?;
    for forbidden in ["build", "workspace", "links"] {
        if package.contains_key(forbidden) {
            return Err(cargo_policy_error(
                path,
                format!("Cargo package.{forbidden} is forbidden."),
            ));
        }
    }
    let package_name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| cargo_policy_error(path, "Cargo package.name is required.".to_string()))?;
    if package_name != expected_package {
        return Err(cargo_policy_error(
            path,
            "Cargo package.name does not match project runtimeModule.cargoPackage.".to_string(),
        ));
    }
    if let Some(lib) = root.get("lib").and_then(toml::Value::as_table) {
        for forbidden in ["path", "crate-type", "proc-macro"] {
            if lib.contains_key(forbidden) {
                return Err(cargo_policy_error(
                    path,
                    format!("Cargo lib.{forbidden} is forbidden."),
                ));
            }
        }
    }
    let dependencies = root
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            cargo_policy_error(path, "Cargo [dependencies] table is required.".to_string())
        })?;
    if dependencies.is_empty() || dependencies.len() > 2 {
        return Err(cargo_policy_error(
            path,
            "Cargo dependencies must contain engine_runtime and optional engine_input only."
                .to_string(),
        ));
    }
    let exact_engine_version = format!("={engine_version}");
    let mut engine_dependencies = BTreeMap::new();
    for (name, dependency) in dependencies {
        if !matches!(name.as_str(), "engine_runtime" | "engine_input") {
            return Err(cargo_policy_error(
                path,
                format!("Cargo dependency '{name}' is outside the v1 allowlist."),
            ));
        }
        let version = dependency.as_str().ok_or_else(|| {
            cargo_policy_error(
                path,
                format!("Cargo dependency '{name}' must be an exact version string."),
            )
        })?;
        if version != exact_engine_version {
            return Err(cargo_policy_error(
                path,
                format!(
                    "Cargo dependency '{name}' must equal project engine version '{exact_engine_version}'."
                ),
            ));
        }
        engine_dependencies.insert(name.clone(), version.to_string());
    }
    if !engine_dependencies.contains_key("engine_runtime") {
        return Err(cargo_policy_error(
            path,
            "Cargo dependency engine_runtime is required.".to_string(),
        ));
    }
    Ok(CargoContract {
        engine_version: engine_version.to_string(),
        engine_dependencies,
    })
}

fn cargo_policy_error(path: &Path, message: String) -> ControlledSourcePatchError {
    ControlledSourcePatchError::new(
        "controlled_source_patch.cargo_policy_rejected",
        message,
        Some(path),
        "Use the restricted package and exact Engine SDK dependency contract.",
    )
}

fn contains_workspace_inheritance(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => {
            table
                .get("workspace")
                .is_some_and(|value| value.as_bool() == Some(true))
                || table.values().any(contains_workspace_inheritance)
        }
        toml::Value::Array(values) => values.iter().any(contains_workspace_inheritance),
        _ => false,
    }
}

fn resolve_engine_sdk(
    locator: &TrustedEngineSdkLocator,
    cargo_contract: &CargoContract,
) -> Result<ResolvedEngineSdk, ControlledSourcePatchError> {
    let root = locator.sdk_root.canonicalize().map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.engine_sdk_unavailable",
            format!("Trusted Engine SDK root is unavailable: {error}"),
            Some(&locator.sdk_root),
            "Select an installed Engine SDK root.",
        )
    })?;
    let engine_runtime_root =
        resolve_sdk_crate(&root, "engine_runtime", &cargo_contract.engine_version)?;
    let engine_input_root =
        resolve_sdk_crate(&root, "engine_input", &cargo_contract.engine_version)?;
    for dependency in cargo_contract.engine_dependencies.keys() {
        if !matches!(dependency.as_str(), "engine_runtime" | "engine_input") {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.engine_sdk_dependency_invalid",
                "Candidate requests an unsupported Engine SDK crate.",
                None,
                "Regenerate the candidate using the v1 dependency allowlist.",
            ));
        }
    }
    Ok(ResolvedEngineSdk {
        root,
        engine_runtime_root,
        engine_input_root,
    })
}

fn resolve_sdk_crate(
    sdk_root: &Path,
    crate_name: &str,
    expected_version: &str,
) -> Result<PathBuf, ControlledSourcePatchError> {
    let crate_root = sdk_root.join("crates").join(crate_name);
    let canonical = crate_root.canonicalize().map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.engine_sdk_crate_unavailable",
            format!("Trusted Engine SDK crate '{crate_name}' is unavailable: {error}"),
            Some(&crate_root),
            "Install a complete matching Engine SDK.",
        )
    })?;
    if canonical.parent().and_then(Path::parent) != Some(sdk_root) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.engine_sdk_crate_escaped",
            format!("Trusted Engine SDK crate '{crate_name}' escaped its SDK root."),
            Some(&canonical),
            "Repair the trusted Engine SDK installation.",
        ));
    }
    let manifest_path = canonical.join("Cargo.toml");
    let manifest_text = read_regular_utf8(&manifest_path)?;
    let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.engine_sdk_manifest_invalid",
            format!("Engine SDK Cargo manifest is invalid: {error}"),
            Some(&manifest_path),
            "Repair the trusted Engine SDK installation.",
        )
    })?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.engine_sdk_manifest_invalid",
                "Engine SDK Cargo manifest has no [package] table.",
                Some(&manifest_path),
                "Repair the trusted Engine SDK installation.",
            )
        })?;
    if package.get("name").and_then(toml::Value::as_str) != Some(crate_name)
        || package.get("version").and_then(toml::Value::as_str) != Some(expected_version)
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.engine_sdk_version_mismatch",
            format!("Engine SDK crate '{crate_name}' does not match version {expected_version}."),
            Some(&manifest_path),
            "Use the Engine SDK version declared by the project manifest.",
        ));
    }
    Ok(canonical)
}

fn copy_runtime_module(
    source: &Path,
    destination: &Path,
) -> Result<(), ControlledSourcePatchError> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.runtime_module_unavailable",
            format!("RuntimeModule cannot be inspected: {error}"),
            Some(source),
            "Restore a regular project-owned RuntimeModule directory.",
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.runtime_module_not_regular",
            "RuntimeModule must be a regular directory, not a link or reparse point.",
            Some(source),
            "Replace it with a project-owned regular directory.",
        ));
    }
    fs::create_dir(destination).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.validation_copy_failed",
            format!("Validation RuntimeModule cannot be created: {error}"),
            Some(destination),
            "Check candidate store permissions.",
        )
    })?;
    copy_regular_tree(source, destination, source)
}

fn copy_regular_tree(
    source_root: &Path,
    destination_root: &Path,
    directory: &Path,
) -> Result<(), ControlledSourcePatchError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.validation_copy_failed",
                format!("RuntimeModule directory cannot be read: {error}"),
                Some(directory),
                "Restore a readable regular RuntimeModule tree.",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.validation_copy_failed",
                format!("RuntimeModule entry cannot be read: {error}"),
                Some(directory),
                "Restore a readable regular RuntimeModule tree.",
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source = entry.path();
        let relative = source.strip_prefix(source_root).map_err(|_| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.validation_copy_escaped",
                "RuntimeModule validation copy escaped its source root.",
                Some(&source),
                "Resolve the project containment violation.",
            )
        })?;
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.validation_copy_failed",
                format!("RuntimeModule metadata cannot be read: {error}"),
                Some(&source),
                "Restore a regular RuntimeModule tree.",
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.runtime_module_link_rejected",
                "RuntimeModule validation does not follow links or reparse points.",
                Some(&source),
                "Replace the link with project-owned regular content.",
            ));
        }
        let destination = destination_root.join(relative);
        if metadata.is_dir() {
            fs::create_dir(&destination).map_err(|error| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.validation_copy_failed",
                    format!("Validation directory cannot be created: {error}"),
                    Some(&destination),
                    "Check candidate store permissions.",
                )
            })?;
            copy_regular_tree(source_root, destination_root, &source)?;
        } else if metadata.is_file() {
            fs::copy(&source, &destination).map_err(|error| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.validation_copy_failed",
                    format!("RuntimeModule file cannot be copied: {error}"),
                    Some(&source),
                    "Check candidate store capacity and permissions.",
                )
            })?;
        } else {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.runtime_module_special_file_rejected",
                "RuntimeModule contains an unsupported special file.",
                Some(&source),
                "Remove the special file from RuntimeModule.",
            ));
        }
    }
    Ok(())
}

fn write_engine_sdk_patch_config(
    runtime_module_root: &Path,
    sdk: &ResolvedEngineSdk,
) -> Result<(), ControlledSourcePatchError> {
    let cargo_dir = runtime_module_root.join(".cargo");
    fs::create_dir(&cargo_dir).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.validation_config_failed",
            format!("Validation Cargo config directory cannot be created: {error}"),
            Some(&cargo_dir),
            "Check candidate store permissions.",
        )
    })?;
    let mut crates_io = toml::map::Map::new();
    for (name, root) in [
        ("engine_runtime", &sdk.engine_runtime_root),
        ("engine_input", &sdk.engine_input_root),
    ] {
        let mut dependency = toml::map::Map::new();
        dependency.insert(
            "path".to_string(),
            toml::Value::String(root.display().to_string()),
        );
        crates_io.insert(name.to_string(), toml::Value::Table(dependency));
    }
    let mut patch = toml::map::Map::new();
    patch.insert("crates-io".to_string(), toml::Value::Table(crates_io));
    let mut root = toml::map::Map::new();
    root.insert("patch".to_string(), toml::Value::Table(patch));
    let text = toml::to_string(&toml::Value::Table(root)).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.validation_config_failed",
            format!("Validation Cargo config cannot be encoded: {error}"),
            Some(&cargo_dir.join("config.toml")),
            "Inspect the trusted Engine SDK config implementation.",
        )
    })?;
    fs::write(cargo_dir.join("config.toml"), text).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.validation_config_failed",
            format!("Validation Cargo config cannot be written: {error}"),
            Some(&cargo_dir.join("config.toml")),
            "Check candidate store permissions.",
        )
    })
}

fn run_cargo_validation(
    candidate: &ControlledSourcePatchCandidate,
    request: &ControlledSourcePatchValidationRequest,
    sdk: &ResolvedEngineSdk,
    validation_root: &Path,
    runtime_module_root: &Path,
    cancellation: Option<&BoundedChildProcessCancellation>,
) -> ControlledSourcePatchValidationReport {
    let process_validation_root = subprocess_path(validation_root);
    let process_runtime_module_root = subprocess_path(runtime_module_root);
    let executable = request
        .cargo_executable
        .clone()
        .or_else(|| std::env::var_os("CARGO").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let timeout = Duration::from_millis(request.step_timeout_ms.max(1).min(MAX_STEP_TIMEOUT_MS));
    let capture_limit = request
        .capture_limit_bytes
        .max(1)
        .min(MAX_CAPTURE_LIMIT_BYTES);
    let mut commands = vec![
        ("cargo_fmt_check", vec!["fmt", "--all", "--", "--check"]),
        (
            "cargo_check_all_targets",
            vec!["check", "--all-targets", "--offline"],
        ),
        ("cargo_test_no_run", vec!["test", "--no-run", "--offline"]),
    ];
    if request.execution_policy == ControlledSourcePatchExecutionPolicy::TrustedLocalExecuteTests {
        commands.push(("cargo_test", vec!["test", "--offline"]));
    }
    let environment = vec![
        (
            OsString::from("CARGO_TARGET_DIR"),
            process_validation_root.join("target").into_os_string(),
        ),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
    ];
    let mut steps = Vec::new();
    let mut diagnostics = Vec::new();
    for (name, args) in commands {
        if cancellation.is_some_and(BoundedChildProcessCancellation::is_cancelled) {
            diagnostics.push(
                ControlledSourcePatchDiagnostic::error(
                    "controlled_source_patch.validation_cancelled",
                    "Validation was cancelled before the next bounded Cargo step.",
                )
                .with_path(CARGO_MANIFEST_PATH)
                .with_next_action("Observe the terminal cancellation receipt before retrying."),
            );
            break;
        }
        let command = std::iter::once(executable.display().to_string())
            .chain(args.iter().map(|value| value.to_string()))
            .collect::<Vec<_>>();
        let process_request = BoundedChildProcessRequest {
            executable: executable.clone(),
            args: args.iter().map(OsString::from).collect(),
            current_dir: process_runtime_module_root.clone(),
            environment: environment.clone(),
            timeout,
            stdout_capture_limit_bytes: capture_limit,
            stderr_capture_limit_bytes: capture_limit,
            priority: runtime_cli::BoundedChildProcessPriority::Normal,
        };
        let process = if let Some(cancellation) = cancellation {
            run_bounded_child_process_cancellable(process_request, cancellation.clone())
        } else {
            run_bounded_child_process(process_request)
        };
        let status = if process.exit_reason == BoundedChildProcessExitReason::Completed
            && process.exit_code == Some(0)
            && process.reader_join_error.is_none()
        {
            ControlledSourcePatchValidationStepStatus::Passed
        } else {
            ControlledSourcePatchValidationStepStatus::Failed
        };
        if status == ControlledSourcePatchValidationStepStatus::Failed {
            let (code, message, next_action) =
                if process.exit_reason == BoundedChildProcessExitReason::Cancelled {
                    (
                        "controlled_source_patch.validation_cancelled",
                        format!("Validation step '{name}' was cancelled and its child was reaped."),
                        "Observe the terminal cancellation receipt before retrying.",
                    )
                } else {
                    (
                        "controlled_source_patch.validation_step_failed",
                        format!("Validation step '{name}' failed."),
                        "Review the bounded Cargo diagnostics and repair the candidate.",
                    )
                };
            diagnostics.push(
                ControlledSourcePatchDiagnostic::error(code, message)
                    .with_path(CARGO_MANIFEST_PATH)
                    .with_next_action(next_action),
            );
        }
        steps.push(ControlledSourcePatchValidationStep {
            name: name.to_string(),
            status,
            command,
            process,
        });
        if status == ControlledSourcePatchValidationStepStatus::Failed {
            break;
        }
    }
    let status = if diagnostics.is_empty() {
        ControlledSourcePatchValidationStatus::Passed
    } else {
        ControlledSourcePatchValidationStatus::Failed
    };
    ControlledSourcePatchValidationReport {
        schema_version: CONTROLLED_SOURCE_PATCH_VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
        status,
        patch_id: candidate.patch_id.clone(),
        patch_digest: candidate.patch_digest.clone(),
        revision_id: candidate.revision.revision_id.clone(),
        base_project_digest: candidate.revision.base_project_digest.clone(),
        candidate_project_digest: candidate.revision.candidate_project_digest.clone(),
        changed_paths: candidate.revision.changed_paths.clone(),
        validation_digest: String::new(),
        execution_policy: request.execution_policy,
        engine_sdk_root: sdk.root.display().to_string(),
        validation_root: validation_root.display().to_string(),
        cleanup_status: "pending".to_string(),
        isolation_notice: "Validation uses an isolated directory and bounded process output; it is not a malicious-code security sandbox. Cargo, the compiler, and explicitly executed tests retain the current user process authority.".to_string(),
        steps,
        diagnostics,
        next_actions: if status == ControlledSourcePatchValidationStatus::Passed {
            vec!["Review the diff and validation evidence before explicit approval.".to_string()]
        } else {
            vec!["Repair the candidate and create a new revision before approval.".to_string()]
        },
    }
}

fn validation_report_digest(
    report: &ControlledSourcePatchValidationReport,
) -> Result<String, ControlledSourcePatchError> {
    let mut input = report.clone();
    input.validation_digest.clear();
    digest_serializable(&input, "validation report")
}

fn validate_passed_report(
    candidate: &ControlledSourcePatchCandidate,
    report: &ControlledSourcePatchValidationReport,
) -> Result<(), ControlledSourcePatchError> {
    if report.schema_version != CONTROLLED_SOURCE_PATCH_VALIDATION_REPORT_SCHEMA_VERSION
        || report.status != ControlledSourcePatchValidationStatus::Passed
        || report.cleanup_status != "removed"
        || report.steps.is_empty()
        || report
            .steps
            .iter()
            .any(|step| step.status != ControlledSourcePatchValidationStepStatus::Passed)
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.validation_not_passed",
            "Apply requires a passed validation report with successful cleanup.",
            None,
            "Validate the exact candidate successfully before approval.",
        ));
    }
    if report.patch_id != candidate.patch_id
        || report.patch_digest != candidate.patch_digest
        || report.revision_id != candidate.revision.revision_id
        || report.base_project_digest != candidate.revision.base_project_digest
        || report.candidate_project_digest != candidate.revision.candidate_project_digest
        || report.changed_paths != candidate.revision.changed_paths
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.validation_binding_mismatch",
            "Validation report does not bind the exact SourcePatch candidate.",
            None,
            "Validate and approve the same candidate revision.",
        ));
    }
    let digest = validation_report_digest(report)?;
    if digest != report.validation_digest {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.validation_digest_mismatch",
            "Validation report content no longer matches its digest.",
            None,
            "Discard the modified report and validate again.",
        ));
    }
    Ok(())
}

fn validate_approval(
    candidate: &ControlledSourcePatchCandidate,
    report: &ControlledSourcePatchValidationReport,
    approval: &ControlledSourcePatchApproval,
) -> Result<(), ControlledSourcePatchError> {
    if approval.schema_version != CONTROLLED_SOURCE_PATCH_APPROVAL_SCHEMA_VERSION
        || approval.patch_id != candidate.patch_id
        || approval.revision_id != candidate.revision.revision_id
        || approval.candidate_project_digest != candidate.revision.candidate_project_digest
        || approval.validation_digest != report.validation_digest
        || approval.approved_by.trim().is_empty()
        || approval.approved_by.len() > 128
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.approval_binding_mismatch",
            "Approval does not bind the exact passed candidate validation.",
            None,
            "Request explicit approval for this revision and validation digest.",
        ));
    }
    Ok(())
}

fn snapshot_before_files(
    scope: &ProjectWriteScope,
    changed_paths: &[String],
) -> Result<Vec<RollbackFileSnapshot>, ControlledSourcePatchError> {
    let mut total = 0_usize;
    let mut snapshots = Vec::with_capacity(changed_paths.len());
    for path in changed_paths {
        let before_bytes = if scope.try_exists(path).map_err(project_write_error)? {
            let bytes = scope.read(path).map_err(project_write_error)?;
            total = total.checked_add(bytes.len()).ok_or_else(|| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.rollback_snapshot_too_large",
                    "Rollback snapshot size overflowed the supported range.",
                    None,
                    "Split the SourcePatch into smaller revisions.",
                )
            })?;
            if total > MAX_ROLLBACK_BYTES {
                return Err(ControlledSourcePatchError::new(
                    "controlled_source_patch.rollback_snapshot_too_large",
                    format!("Rollback snapshot exceeds {MAX_ROLLBACK_BYTES} bytes."),
                    None,
                    "Split the SourcePatch into smaller revisions.",
                ));
            }
            Some(bytes)
        } else {
            None
        };
        snapshots.push(RollbackFileSnapshot {
            path: path.clone(),
            before_bytes,
        });
    }
    Ok(snapshots)
}

fn apply_candidate_files(
    scope: &ProjectWriteScope,
    revision: &CandidateProjectRevision,
) -> Result<(), ControlledSourcePatchError> {
    let candidate_root = Path::new(&revision.candidate_root);
    for path in &revision.changed_paths {
        let source = candidate_root.join(path);
        if source.exists() {
            let bytes = fs::read(&source).map_err(|error| {
                ControlledSourcePatchError::new(
                    "controlled_source_patch.candidate_read_failed",
                    format!("Validated candidate file cannot be read: {error}"),
                    Some(&source),
                    "Reject the candidate and preserve the project base.",
                )
            })?;
            scope
                .write_atomic(path, &bytes)
                .map_err(project_write_error)?;
        } else {
            scope.remove_file(path).map_err(project_write_error)?;
        }
    }
    Ok(())
}

fn restore_snapshots(
    scope: &ProjectWriteScope,
    snapshots: &[RollbackFileSnapshot],
) -> Result<(), ControlledSourcePatchError> {
    for snapshot in snapshots.iter().rev() {
        if let Some(bytes) = &snapshot.before_bytes {
            scope
                .write_atomic(&snapshot.path, bytes)
                .map_err(project_write_error)?;
        } else {
            scope
                .remove_file(&snapshot.path)
                .map_err(project_write_error)?;
        }
    }
    Ok(())
}

fn fail_apply_and_restore(
    cause: ControlledSourcePatchError,
    scope: &ProjectWriteScope,
    record: &RollbackRecord,
    store_root: &Path,
    rollback_name: &str,
    revision: &CandidateProjectRevision,
) -> Result<ControlledSourcePatchApplyReceipt, ControlledSourcePatchError> {
    if let Err(restore_error) = restore_snapshots(scope, &record.snapshots) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.apply_rollback_failed",
            format!("Apply failed ({cause}); automatic restoration also failed: {restore_error}"),
            Some(scope.display_root()),
            "Preserve the rollback record and recover with a trusted maintainer.",
        ));
    }
    let restored = CandidateProjectRevisionStore::verify_base(revision, scope.display_root())
        .map_err(candidate_error);
    if !matches!(
        restored,
        Ok(ref verification)
            if verification.status == CandidateBaseVerificationStatus::Matched
                && verification.actual_digest == record.before_project_digest
    ) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.apply_rollback_digest_mismatch",
            "Apply failed and restoration did not reproduce the recorded before digest.",
            Some(scope.display_root()),
            "Preserve the rollback record and recover with a trusted maintainer.",
        ));
    }
    let store_scope = ProjectWriteScope::open(store_root).map_err(project_write_error)?;
    store_scope
        .remove_file(rollback_name)
        .map_err(project_write_error)?;
    Err(ControlledSourcePatchError::new(
        "controlled_source_patch.apply_failed_restored",
        format!("SourcePatch apply failed and the before snapshot was restored: {cause}"),
        Some(scope.display_root()),
        "Repair the candidate and create a new validated revision.",
    ))
}

fn rollback_record_digest(record: &RollbackRecord) -> Result<String, ControlledSourcePatchError> {
    let mut input = record.clone();
    input.record_digest.clear();
    input.receipt_binding_digest.clear();
    digest_serializable(&input, "rollback record")
}

fn receipt_binding_digest(
    patch_id: &str,
    revision: &CandidateProjectRevision,
    validation_digest: &str,
    rollback_path: &Path,
    rollback_record_digest: &str,
) -> Result<String, ControlledSourcePatchError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Binding<'a> {
        patch_id: &'a str,
        revision_id: &'a str,
        before_project_digest: &'a str,
        applied_project_digest: &'a str,
        validation_digest: &'a str,
        changed_paths: &'a [String],
        rollback_record_path: String,
        rollback_record_digest: &'a str,
    }
    digest_serializable(
        &Binding {
            patch_id,
            revision_id: &revision.revision_id,
            before_project_digest: &revision.base_project_digest,
            applied_project_digest: &revision.candidate_project_digest,
            validation_digest,
            changed_paths: &revision.changed_paths,
            rollback_record_path: rollback_path.display().to_string(),
            rollback_record_digest,
        },
        "apply receipt binding",
    )
}

fn validate_apply_receipt(
    receipt: &ControlledSourcePatchApplyReceipt,
    project_root: &Path,
) -> Result<(), ControlledSourcePatchError> {
    if receipt.schema_version != CONTROLLED_SOURCE_PATCH_APPLY_RECEIPT_SCHEMA_VERSION
        || receipt.before_project_digest != receipt.revision.base_project_digest
        || receipt.applied_project_digest != receipt.revision.candidate_project_digest
        || receipt.changed_paths != receipt.revision.changed_paths
        || validate_digest(&receipt.rollback_record_digest, "rollback record digest").is_err()
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.apply_receipt_invalid",
            "Apply receipt does not match its CandidateProjectRevision.",
            Some(project_root),
            "Use the original apply receipt and project root.",
        ));
    }
    let expected = receipt_binding_digest(
        &receipt.patch_id,
        &receipt.revision,
        &receipt.validation_digest,
        Path::new(&receipt.rollback_record_path),
        &receipt.rollback_record_digest,
    )?;
    if expected != receipt.receipt_binding_digest {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.apply_receipt_binding_mismatch",
            "Apply receipt binding digest is invalid.",
            None,
            "Use the original unmodified apply receipt.",
        ));
    }
    Ok(())
}

fn validate_rollback_record(
    receipt: &ControlledSourcePatchApplyReceipt,
    record: &RollbackRecord,
) -> Result<(), ControlledSourcePatchError> {
    if record.schema_version != ROLLBACK_RECORD_SCHEMA_VERSION
        || record.patch_id != receipt.patch_id
        || record.revision_id != receipt.revision.revision_id
        || record.before_project_digest != receipt.before_project_digest
        || record.applied_project_digest != receipt.applied_project_digest
        || record.changed_paths != receipt.changed_paths
        || record.record_digest != receipt.rollback_record_digest
        || rollback_record_digest(record).as_deref() != Ok(receipt.rollback_record_digest.as_str())
        || record.receipt_binding_digest != receipt.receipt_binding_digest
        || record.snapshots.len() != receipt.changed_paths.len()
        || record
            .snapshots
            .iter()
            .map(|snapshot| &snapshot.path)
            .ne(receipt.changed_paths.iter())
    {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.rollback_record_binding_mismatch",
            "Rollback record does not bind the exact apply receipt.",
            Some(Path::new(&receipt.rollback_record_path)),
            "Preserve the record and recover with a trusted maintainer.",
        ));
    }
    Ok(())
}

fn canonical_store_root(
    candidate: &ControlledSourcePatchCandidate,
) -> Result<PathBuf, ControlledSourcePatchError> {
    Path::new(&candidate.candidate_store_root)
        .canonicalize()
        .map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.candidate_store_unavailable",
                format!("Candidate store is unavailable: {error}"),
                Some(Path::new(&candidate.candidate_store_root)),
                "Restore the candidate store before continuing.",
            )
        })
}

fn validation_root(store_root: &Path, _revision_id: &str) -> PathBuf {
    let sequence = VALIDATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    store_root.join(format!(".cspv-{}-{sequence}", std::process::id()))
}

#[cfg(windows)]
fn subprocess_path(path: &Path) -> PathBuf {
    let display = path.to_string_lossy();
    if let Some(rest) = display.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = display.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

#[cfg(not(windows))]
fn subprocess_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn rollback_record_name(revision_id: &str) -> String {
    format!(".{revision_id}.source-patch-rollback.json")
}

fn validate_opaque_id(value: &str, role: &str) -> Result<(), ControlledSourcePatchError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(ControlledSourcePatchError::new(
            "controlled_source_patch.id_invalid",
            format!("{role} must contain 1-96 ASCII letters, digits, '-' or '_'."),
            None,
            "Generate a canonical opaque identifier.",
        ))
    } else {
        Ok(())
    }
}

fn validate_digest(value: &str, role: &str) -> Result<(), ControlledSourcePatchError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(ControlledSourcePatchError::new(
            "controlled_source_patch.digest_invalid",
            format!("{role} is not a canonical SHA-256 digest."),
            None,
            "Reject the invalid record and regenerate it.",
        ))
    }
}

fn digest_serializable(
    value: &impl Serialize,
    role: &str,
) -> Result<String, ControlledSourcePatchError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            ControlledSourcePatchError::new(
                "controlled_source_patch.digest_encode_failed",
                format!("{role} cannot be encoded for deterministic digest: {error}"),
                None,
                "Inspect the structured record implementation.",
            )
        })
}

fn validate_cargo_package_name(name: &str) -> Result<(), ControlledSourcePatchError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(ControlledSourcePatchError::new(
            "controlled_source_patch.cargo_package_invalid",
            "Project runtime Cargo package name is invalid.",
            None,
            "Use 1-64 ASCII letters, digits, '-' or '_'.",
        ))
    } else {
        Ok(())
    }
}

fn validate_portable_path(path: &ProjectRelativePath) -> Result<(), ControlledSourcePatchError> {
    for component in path.as_path().components() {
        let Component::Normal(value) = component else {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.path_not_portable",
                "SourcePatch path contains a non-portable component.",
                Some(path.as_path()),
                "Use canonical portable project path components.",
            ));
        };
        let Some(value) = value.to_str() else {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.path_not_utf8",
                "SourcePatch path contains non-UTF-8 text.",
                Some(path.as_path()),
                "Use UTF-8 project paths.",
            ));
        };
        let upper = value.to_ascii_uppercase();
        let stem = upper.split('.').next().unwrap_or(&upper);
        let reserved = matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
            || stem.strip_prefix("COM").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
            || stem.strip_prefix("LPT").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
        if value.contains(':') || value.ends_with(['.', ' ']) || reserved {
            return Err(ControlledSourcePatchError::new(
                "controlled_source_patch.path_not_portable",
                "SourcePatch path uses a Windows-reserved or ambiguous component.",
                Some(path.as_path()),
                "Rename the path using portable project path syntax.",
            ));
        }
    }
    Ok(())
}

fn is_source_patch_path_allowed(path: &str) -> bool {
    if matches!(path, PROJECT_MANIFEST_PATH | CARGO_MANIFEST_PATH) {
        return true;
    }
    let Some(relative) = path.strip_prefix("RuntimeModule/") else {
        return false;
    };
    let in_rust_root = relative.starts_with("src/") || relative.starts_with("tests/");
    in_rust_root && relative.ends_with(".rs")
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn read_regular_utf8(path: &Path) -> Result<String, ControlledSourcePatchError> {
    ensure_regular_file(path)?;
    fs::read_to_string(path).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.utf8_file_required",
            format!("Controlled project file must be readable UTF-8: {error}"),
            Some(path),
            "Write a regular UTF-8 project source file.",
        )
    })
}

fn ensure_regular_file(path: &Path) -> Result<(), ControlledSourcePatchError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ControlledSourcePatchError::new(
            "controlled_source_patch.required_file_unavailable",
            format!("Required project file is unavailable: {error}"),
            Some(path),
            "Create the required regular project source file.",
        )
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(ControlledSourcePatchError::new(
            "controlled_source_patch.required_file_not_regular",
            "Required project source must be a regular file, not a link or reparse point.",
            Some(path),
            "Replace it with a project-owned regular file.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn candidate_error(error: crate::CandidateProjectRevisionError) -> ControlledSourcePatchError {
    ControlledSourcePatchError {
        code: error.code,
        message: error.message,
        path: error.path,
        next_action: error.next_action,
    }
}

fn project_write_error(error: crate::ProjectWriteError) -> ControlledSourcePatchError {
    ControlledSourcePatchError::new(
        error.code,
        error.to_string(),
        error.relative_path.as_deref().map(Path::new),
        "Resolve the project write containment error and retry.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    mod policy {
        use super::*;

        #[test]
        fn rejects_paths_outside_allowlist_and_windows_ambiguous_names() {
            let mut patch = source_patch("pub fn value() -> u32 { 7 }\n", None);
            patch.operations = vec![ControlledSourcePatchOperation::CreateOrReplace {
                path: "Scenes/Main.scene.json".to_string(),
                text: "{}".to_string(),
            }];
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.path_not_allowed");

            patch.operations = vec![ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/src/CON.rs".to_string(),
                text: "pub fn value() {}".to_string(),
            }];
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.path_not_portable");
        }

        #[test]
        fn rejects_unsorted_case_colliding_and_required_deletes() {
            let mut patch = source_patch("pub fn value() -> u32 { 7 }\n", None);
            patch.operations.swap(0, 1);
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(
                error.code,
                "controlled_source_patch.operations_not_sorted_unique"
            );

            patch.operations = vec![
                ControlledSourcePatchOperation::CreateOrReplace {
                    path: "RuntimeModule/src/Foo.rs".to_string(),
                    text: "pub fn upper() {}".to_string(),
                },
                ControlledSourcePatchOperation::CreateOrReplace {
                    path: "RuntimeModule/src/foo.rs".to_string(),
                    text: "pub fn lower() {}".to_string(),
                },
            ];
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.path_case_collision");

            patch.operations = vec![ControlledSourcePatchOperation::Delete {
                path: RUNTIME_LIB_PATH.to_string(),
            }];
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(
                error.code,
                "controlled_source_patch.required_file_delete_rejected"
            );
        }

        #[test]
        fn rejects_file_and_total_text_limits() {
            let mut patch = source_patch("pub fn value() -> u32 { 7 }\n", None);
            patch.operations = vec![ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/src/large.rs".to_string(),
                text: "x".repeat(MAX_FILE_TEXT_BYTES + 1),
            }];
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.file_too_large");

            patch.operations = (0..5)
                .map(|index| ControlledSourcePatchOperation::CreateOrReplace {
                    path: format!("RuntimeModule/src/file_{index}.rs"),
                    text: "x".repeat(MAX_FILE_TEXT_BYTES),
                })
                .collect();
            let error = validate_source_patch(&patch).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.total_text_too_large");
        }

        #[test]
        fn rejects_cargo_build_workspace_target_and_external_dependencies() {
            for forbidden in [
                "build = \"build.rs\"",
                "workspace = \"../..\"",
                "[target.'cfg(windows)'.dependencies]",
                "serde = \"=1.0.0\"",
                "engine_runtime = { version = \"=0.0.2\", path = \"../engine\" }",
                "crate-type = [\"dylib\"]",
            ] {
                let (root, store) = fixture_project("cargo-policy");
                let mut patch = source_patch("pub fn value() -> u32 { 7 }\n", None);
                let cargo = patch
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        ControlledSourcePatchOperation::CreateOrReplace { path, text }
                            if path == CARGO_MANIFEST_PATH =>
                        {
                            Some(text)
                        }
                        _ => None,
                    })
                    .unwrap();
                if forbidden.starts_with("serde") || forbidden.starts_with("engine_runtime = {") {
                    cargo.push_str(forbidden);
                    cargo.push('\n');
                } else if forbidden.starts_with("crate-type") {
                    cargo.push_str("\n[lib]\n");
                    cargo.push_str(forbidden);
                    cargo.push('\n');
                } else if forbidden.starts_with('[') {
                    cargo.push('\n');
                    cargo.push_str(forbidden);
                    cargo.push_str("\nengine_runtime = \"=0.0.2\"\n");
                } else {
                    cargo.push_str(forbidden);
                    cargo.push('\n');
                }
                let error = ControlledSourcePatch::prepare(ControlledSourcePatchPrepareRequest {
                    revision_id: "revision_cargo_policy".to_string(),
                    project_root: root,
                    candidate_store_root: store.clone(),
                    source_patch: patch,
                })
                .unwrap_err();
                assert!(
                    matches!(
                        error.code.as_str(),
                        "controlled_source_patch.cargo_policy_rejected"
                            | "controlled_source_patch.cargo_manifest_invalid"
                    ),
                    "unexpected error for {forbidden}: {error}"
                );
                assert!(!store.join("revision_cargo_policy").exists());
            }
        }
    }

    mod prepare {
        use super::*;

        #[test]
        fn stages_project_rust_candidate_without_mutating_base() {
            let (root, store) = fixture_project("prepare");
            let before = fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap();
            let candidate = prepare_candidate(&root, &store, "revision_prepare", None);

            assert_eq!(candidate.patch_id, "patch_001");
            assert_eq!(
                candidate.revision.changed_paths,
                vec![
                    CARGO_MANIFEST_PATH.to_string(),
                    RUNTIME_LIB_PATH.to_string(),
                    PROJECT_MANIFEST_PATH.to_string(),
                ]
            );
            assert_eq!(fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap(), before);
            assert!(!root.join(CARGO_MANIFEST_PATH).exists());
            assert!(Path::new(&candidate.revision.candidate_root)
                .join(CARGO_MANIFEST_PATH)
                .is_file());
            assert_eq!(
                digest_serializable(&candidate.source_patch, "source patch").unwrap(),
                candidate.patch_digest
            );
        }

        #[test]
        fn rejects_invalid_final_contract_and_cleans_candidate() {
            let (root, store) = fixture_project("prepare-cleanup");
            let mut patch = source_patch("pub fn value() -> u32 { 7 }\n", None);
            let manifest = patch
                .operations
                .iter_mut()
                .find_map(|operation| match operation {
                    ControlledSourcePatchOperation::CreateOrReplace { path, text }
                        if path == PROJECT_MANIFEST_PATH =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .unwrap();
            *manifest = "{}".to_string();

            let error = ControlledSourcePatch::prepare(ControlledSourcePatchPrepareRequest {
                revision_id: "revision_prepare_cleanup".to_string(),
                project_root: root,
                candidate_store_root: store.clone(),
                source_patch: patch,
            })
            .unwrap_err();

            assert_eq!(
                error.code,
                "controlled_source_patch.project_manifest_invalid"
            );
            assert!(!store.join("revision_prepare_cleanup").exists());
        }

        #[test]
        fn serialized_candidate_rejects_patch_and_changed_path_tampering() {
            let (root, store) = fixture_project("candidate-tampering");
            let mut candidate =
                prepare_candidate(&root, &store, "revision_candidate_tampering", None);
            candidate.source_patch.patch_id = "different_patch".to_string();
            let error = validate_candidate_record(&candidate).unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.patch_binding_mismatch");

            candidate.source_patch.patch_id = candidate.patch_id.clone();
            candidate.patch_digest =
                digest_serializable(&candidate.source_patch, "source patch").unwrap();
            candidate
                .revision
                .changed_paths
                .push("RuntimeModule/src/unrequested.rs".to_string());
            let error = validate_candidate_record(&candidate).unwrap_err();
            assert_eq!(
                error.code,
                "controlled_source_patch.changed_paths_outside_request"
            );
        }
    }

    mod validation {
        use super::*;

        #[test]
        fn validates_with_isolated_target_and_cleans_validation_root() {
            let (root, store) = fixture_project("validation");
            let sdk = fixture_engine_sdk("validation");
            let candidate = prepare_candidate(
                &root,
                &store,
                "revision_validation",
                Some("#[test]\nfn compiled_only() {\n    assert_eq!(2 + 2, 4);\n}\n"),
            );

            let report = ControlledSourcePatch::validate(
                &candidate,
                &ControlledSourcePatchValidationRequest::compile_tests_only(&sdk),
            )
            .unwrap();

            assert_eq!(
                report.status,
                ControlledSourcePatchValidationStatus::Passed,
                "{report:#?}"
            );
            assert_eq!(report.cleanup_status, "removed");
            assert_eq!(report.steps.len(), 3);
            assert!(!Path::new(&report.validation_root).exists());
            assert!(report
                .isolation_notice
                .contains("not a malicious-code security sandbox"));
            assert_eq!(
                validation_report_digest(&report).unwrap(),
                report.validation_digest
            );
        }

        #[test]
        fn default_compiles_tests_without_executing_them_and_trusted_policy_executes() {
            let (root, store) = fixture_project("validation-policy");
            let sdk = fixture_engine_sdk("validation-policy");
            let candidate = prepare_candidate(
                &root,
                &store,
                "revision_validation_policy",
                Some("#[test]\nfn must_not_run_by_default() {\n    panic!(\"executed\");\n}\n"),
            );

            let compile_only = ControlledSourcePatch::validate(
                &candidate,
                &ControlledSourcePatchValidationRequest::compile_tests_only(&sdk),
            )
            .unwrap();
            assert_eq!(
                compile_only.status,
                ControlledSourcePatchValidationStatus::Passed,
                "{compile_only:#?}"
            );
            assert_eq!(compile_only.steps.len(), 3);

            let mut trusted = ControlledSourcePatchValidationRequest::compile_tests_only(&sdk);
            trusted.execution_policy =
                ControlledSourcePatchExecutionPolicy::TrustedLocalExecuteTests;
            let executed = ControlledSourcePatch::validate(&candidate, &trusted).unwrap();
            assert_eq!(
                executed.status,
                ControlledSourcePatchValidationStatus::Failed
            );
            assert_eq!(executed.steps.len(), 4);
            assert_eq!(
                executed.steps.last().unwrap().name,
                "cargo_test".to_string()
            );
            assert!(!Path::new(&executed.validation_root).exists());
        }

        #[test]
        fn rejects_base_drift_before_creating_validation_copy() {
            let (root, store) = fixture_project("validation-drift");
            let sdk = fixture_engine_sdk("validation-drift");
            let candidate = prepare_candidate(&root, &store, "revision_validation_drift", None);
            fs::write(root.join("drift.txt"), b"drift").unwrap();

            let error = ControlledSourcePatch::validate(
                &candidate,
                &ControlledSourcePatchValidationRequest::compile_tests_only(&sdk),
            )
            .unwrap_err();

            assert_eq!(error.code, "controlled_source_patch.base_drifted");
            assert_eq!(fs::read_dir(&store).unwrap().count(), 1);
        }

        #[test]
        #[ignore = "local trusted workspace Engine SDK smoke"]
        fn validates_against_workspace_engine_sdk() {
            let (root, store) = fixture_project("workspace-sdk-validation");
            let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .unwrap()
                .to_path_buf();
            let candidate =
                prepare_candidate(&root, &store, "revision_workspace_sdk_validation", None);

            let report = ControlledSourcePatch::validate(
                &candidate,
                &ControlledSourcePatchValidationRequest::compile_tests_only(&sdk),
            )
            .unwrap();

            assert_eq!(
                report.status,
                ControlledSourcePatchValidationStatus::Passed,
                "{report:#?}"
            );
            assert_eq!(report.cleanup_status, "removed");
        }
    }

    mod apply {
        use super::*;

        #[test]
        fn applies_only_validated_candidate_and_persists_rollback_record() {
            let (root, store) = fixture_project("apply");
            let candidate = prepare_candidate(&root, &store, "revision_apply", None);
            let report = passed_report(&candidate);
            let approval = approval(&candidate, &report);

            let receipt = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report,
                approval,
            })
            .unwrap();

            assert_eq!(
                fs::read_to_string(root.join(RUNTIME_LIB_PATH)).unwrap(),
                "pub fn value() -> u32 {\n    7\n}\n"
            );
            assert!(Path::new(&receipt.rollback_record_path).is_file());
            let verification =
                CandidateProjectRevisionStore::verify_base(&candidate.revision, &root).unwrap();
            assert_eq!(
                verification.actual_digest,
                candidate.revision.candidate_project_digest
            );
        }

        #[test]
        fn rejects_approval_replay_and_post_validation_base_drift() {
            let (root, store) = fixture_project("apply-reject");
            let candidate = prepare_candidate(&root, &store, "revision_apply_reject", None);
            let report = passed_report(&candidate);
            let mut wrong_approval = approval(&candidate, &report);
            wrong_approval.validation_digest = format!("sha256:{}", "0".repeat(64));
            let error = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report.clone(),
                approval: wrong_approval,
            })
            .unwrap_err();
            assert_eq!(
                error.code,
                "controlled_source_patch.approval_binding_mismatch"
            );

            fs::write(root.join("drift.txt"), b"drift").unwrap();
            let error = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report.clone(),
                approval: approval(&candidate, &report),
            })
            .unwrap_err();
            assert_eq!(error.code, "controlled_source_patch.apply_base_drifted");
            assert!(!root.join(RUNTIME_LIB_PATH).exists());
        }
    }

    mod rollback {
        use super::*;

        #[test]
        fn restores_before_revision_and_removes_rollback_record() {
            let (root, store) = fixture_project("rollback");
            let before_manifest = fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap();
            let candidate = prepare_candidate(&root, &store, "revision_rollback", None);
            let report = passed_report(&candidate);
            let receipt = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report.clone(),
                approval: approval(&candidate, &report),
            })
            .unwrap();

            let rollback = ControlledSourcePatch::rollback(&receipt, &root).unwrap();

            assert!(rollback.rollback_record_removed);
            assert_eq!(
                fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap(),
                before_manifest
            );
            assert!(!root.join(CARGO_MANIFEST_PATH).exists());
            assert!(!root.join(RUNTIME_LIB_PATH).exists());
            assert!(!Path::new(&receipt.rollback_record_path).exists());
            let verification =
                CandidateProjectRevisionStore::verify_base(&candidate.revision, &root).unwrap();
            assert_eq!(
                verification.status,
                CandidateBaseVerificationStatus::Matched
            );
        }

        #[test]
        fn refuses_to_overwrite_project_drift_after_apply() {
            let (root, store) = fixture_project("rollback-drift");
            let candidate = prepare_candidate(&root, &store, "revision_rollback_drift", None);
            let report = passed_report(&candidate);
            let receipt = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report.clone(),
                approval: approval(&candidate, &report),
            })
            .unwrap();
            fs::write(root.join("manual-change.txt"), b"keep me").unwrap();

            let error = ControlledSourcePatch::rollback(&receipt, &root).unwrap_err();

            assert_eq!(
                error.code,
                "controlled_source_patch.rollback_project_drifted"
            );
            assert_eq!(
                fs::read(root.join("manual-change.txt")).unwrap(),
                b"keep me"
            );
            assert!(Path::new(&receipt.rollback_record_path).exists());
        }

        #[test]
        fn rejects_tampered_rollback_snapshot_before_writing_project() {
            let (root, store) = fixture_project("rollback-record-tampering");
            let candidate =
                prepare_candidate(&root, &store, "revision_rollback_record_tampering", None);
            let report = passed_report(&candidate);
            let receipt = ControlledSourcePatch::apply(ControlledSourcePatchApplyRequest {
                candidate: candidate.clone(),
                validation_report: report.clone(),
                approval: approval(&candidate, &report),
            })
            .unwrap();
            let applied_manifest = fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap();
            let rollback_path = Path::new(&receipt.rollback_record_path);
            let mut record: RollbackRecord =
                serde_json::from_slice(&fs::read(rollback_path).unwrap()).unwrap();
            record.snapshots[0].before_bytes = Some(b"tampered snapshot".to_vec());
            fs::write(rollback_path, serde_json::to_vec(&record).unwrap()).unwrap();

            let error = ControlledSourcePatch::rollback(&receipt, &root).unwrap_err();

            assert_eq!(
                error.code,
                "controlled_source_patch.rollback_record_binding_mismatch"
            );
            assert_eq!(
                fs::read(root.join(PROJECT_MANIFEST_PATH)).unwrap(),
                applied_manifest
            );
        }
    }

    fn fixture_project(label: &str) -> (PathBuf, PathBuf) {
        let root = unique_temp_dir(&format!("project-{label}"));
        let store = unique_temp_dir(&format!("store-{label}"));
        fs::create_dir_all(root.join("Scenes")).unwrap();
        fs::create_dir_all(root.join("Settings")).unwrap();
        fs::create_dir_all(&store).unwrap();
        fs::write(root.join("Scenes/Main.scene.json"), b"{}").unwrap();
        fs::write(
            root.join(PROJECT_MANIFEST_PATH),
            serde_json::to_vec_pretty(&base_manifest()).unwrap(),
        )
        .unwrap();
        (root, store)
    }

    fn base_manifest() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": PROJECT_MANIFEST_SCHEMA_VERSION,
            "projectId": "project-test",
            "projectName": "Test Project",
            "engineVersion": "0.0.2",
            "createdAt": "2026-07-14T00:00:00Z",
            "lastOpenedAt": null,
            "defaultScene": "Scenes/Main.scene.json",
            "assetRoot": "Assets",
            "settingsVersion": "aife-project-settings.v1",
            "runtimeModule": {
                "sourceKind": "builtInEmpty",
                "moduleId": "engine.empty.runtime",
                "interfaceVersion": PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
                "cargoManifest": CARGO_MANIFEST_PATH,
                "cargoPackage": "empty_project_runtime",
                "playerBinary": "empty_project_player"
            }
        })
    }

    fn project_rust_manifest_text() -> String {
        let mut manifest = base_manifest();
        manifest["runtimeModule"] = serde_json::json!({
            "sourceKind": "projectRust",
            "moduleId": "test.project.runtime",
            "interfaceVersion": PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
            "cargoManifest": CARGO_MANIFEST_PATH,
            "cargoPackage": "test_project_runtime",
            "playerBinary": "test_project_player"
        });
        serde_json::to_string_pretty(&manifest).unwrap()
    }

    fn source_patch(runtime_lib: &str, test_source: Option<&str>) -> ControlledSourcePatchDocument {
        let mut operations = vec![
            ControlledSourcePatchOperation::CreateOrReplace {
                path: CARGO_MANIFEST_PATH.to_string(),
                text: "[package]\nname = \"test_project_runtime\"\nversion = \"0.0.2\"\nedition = \"2021\"\n\n[dependencies]\nengine_runtime = \"=0.0.2\"\n".to_string(),
            },
            ControlledSourcePatchOperation::CreateOrReplace {
                path: RUNTIME_LIB_PATH.to_string(),
                text: runtime_lib.to_string(),
            },
        ];
        if let Some(test_source) = test_source {
            operations.push(ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/tests/controlled.rs".to_string(),
                text: test_source.to_string(),
            });
        }
        operations.push(ControlledSourcePatchOperation::CreateOrReplace {
            path: PROJECT_MANIFEST_PATH.to_string(),
            text: project_rust_manifest_text(),
        });
        ControlledSourcePatchDocument {
            schema_version: CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION.to_string(),
            patch_id: "patch_001".to_string(),
            operations,
        }
    }

    fn prepare_candidate(
        root: &Path,
        store: &Path,
        revision_id: &str,
        test_source: Option<&str>,
    ) -> ControlledSourcePatchCandidate {
        ControlledSourcePatch::prepare(ControlledSourcePatchPrepareRequest {
            revision_id: revision_id.to_string(),
            project_root: root.to_path_buf(),
            candidate_store_root: store.to_path_buf(),
            source_patch: source_patch("pub fn value() -> u32 {\n    7\n}\n", test_source),
        })
        .unwrap()
    }

    fn fixture_engine_sdk(label: &str) -> PathBuf {
        let root = unique_temp_dir(&format!("sdk-{label}"));
        for name in ["engine_runtime", "engine_input"] {
            let crate_root = root.join("crates").join(name);
            fs::create_dir_all(crate_root.join("src")).unwrap();
            fs::write(
                crate_root.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"0.0.2\"\nedition = \"2021\"\n"),
            )
            .unwrap();
            fs::write(crate_root.join("src/lib.rs"), "pub fn marker() {}\n").unwrap();
        }
        root
    }

    fn passed_report(
        candidate: &ControlledSourcePatchCandidate,
    ) -> ControlledSourcePatchValidationReport {
        let process = BoundedChildProcessResult {
            process_id: Some(1),
            exit_reason: BoundedChildProcessExitReason::Completed,
            exit_code: Some(0),
            elapsed_ms: 1,
            stdout_summary: String::new(),
            stderr_summary: String::new(),
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            spawn_error: None,
            kill_error: None,
            wait_error: None,
            reader_join_error: None,
            ownership: runtime_cli::BoundedProcessOwnershipEvidence::default(),
            priority: runtime_cli::BoundedChildProcessPriorityEvidence::default(),
        };
        let mut report = ControlledSourcePatchValidationReport {
            schema_version: CONTROLLED_SOURCE_PATCH_VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
            status: ControlledSourcePatchValidationStatus::Passed,
            patch_id: candidate.patch_id.clone(),
            patch_digest: candidate.patch_digest.clone(),
            revision_id: candidate.revision.revision_id.clone(),
            base_project_digest: candidate.revision.base_project_digest.clone(),
            candidate_project_digest: candidate.revision.candidate_project_digest.clone(),
            changed_paths: candidate.revision.changed_paths.clone(),
            validation_digest: String::new(),
            execution_policy: ControlledSourcePatchExecutionPolicy::CompileTestsOnly,
            engine_sdk_root: "trusted-test-sdk".to_string(),
            validation_root: "removed-test-validation-root".to_string(),
            cleanup_status: "removed".to_string(),
            isolation_notice: "not a malicious-code security sandbox".to_string(),
            steps: vec![ControlledSourcePatchValidationStep {
                name: "cargo_test_no_run".to_string(),
                status: ControlledSourcePatchValidationStepStatus::Passed,
                command: vec!["cargo".to_string(), "test".to_string()],
                process,
            }],
            diagnostics: Vec::new(),
            next_actions: vec!["Review before approval.".to_string()],
        };
        report.validation_digest = validation_report_digest(&report).unwrap();
        report
    }

    fn approval(
        candidate: &ControlledSourcePatchCandidate,
        report: &ControlledSourcePatchValidationReport,
    ) -> ControlledSourcePatchApproval {
        ControlledSourcePatchApproval {
            schema_version: CONTROLLED_SOURCE_PATCH_APPROVAL_SCHEMA_VERSION.to_string(),
            patch_id: candidate.patch_id.clone(),
            revision_id: candidate.revision.revision_id.clone(),
            candidate_project_digest: candidate.revision.candidate_project_digest.clone(),
            validation_digest: report.validation_digest.clone(),
            approved_by: "local-test-maintainer".to_string(),
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aife-controlled-source-patch-{label}-{}-{stamp}",
            std::process::id()
        ))
    }
}
