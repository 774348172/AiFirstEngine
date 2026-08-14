use crate::project_editor_composition_artifact::{
    atomic_write_json, directory_size, generated_artifact_executable_name, is_link_or_reparse,
    load_cached_artifact, publish_directory, remove_owned_cache_entry, touch_cache_entry,
    CACHE_ROOT_NAME,
};
use crate::{
    ProjectEditorCompositionArtifact, ProjectEditorCompositionBuildReport,
    ProjectEditorCompositionBuildStatus, ProjectEditorCompositionDiagnostic,
    ProjectEditorCompositionIdentity, ProjectEditorCompositionPromotionBackupStatus,
    ProjectEditorCompositionPromotionCleanupStatus, ProjectEditorCompositionPromotionReport,
    ProjectEditorCompositionPromotionRequest, ProjectEditorCompositionPromotionRollbackStatus,
    ProjectEditorCompositionPromotionStage, ProjectEditorCompositionPromotionStatus,
    ProjectEditorCompositionQualificationKind, ProjectEditorCompositionQualificationSeal,
    PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static PROMOTION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct PromotionError {
    code: String,
    stage: ProjectEditorCompositionPromotionStage,
    message: String,
    path: Option<PathBuf>,
    next_action: String,
}

impl PromotionError {
    fn new(
        code: &str,
        stage: ProjectEditorCompositionPromotionStage,
        message: impl Into<String>,
        path: Option<&Path>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            stage,
            message: message.into(),
            path: path.map(Path::to_path_buf),
            next_action: next_action.into(),
        }
    }
}

#[derive(Debug)]
struct ValidatedPromotionArtifact {
    executable_path: PathBuf,
    descriptor_path: PathBuf,
    build_report_path: PathBuf,
    seal_path: PathBuf,
    qualification_report_path: PathBuf,
    executable_hash: String,
    descriptor_hash: String,
    build_report_hash: String,
    qualification_report_digest: String,
    identity_digest: String,
    resolved_identity: crate::ProjectEditorCompositionResolvedIdentity,
}

#[derive(Debug, Default, Clone, Copy)]
struct PromotionFaults {
    mutate_source_after_validation: bool,
    fail_before_publish: bool,
    fail_after_publish: bool,
    fail_rollback: bool,
}

impl ProjectEditorCompositionArtifact {
    pub fn seal_qualification(
        artifact: &ProjectEditorCompositionArtifact,
        qualification_report_path: &Path,
        seal_path: &Path,
    ) -> Result<ProjectEditorCompositionQualificationSeal, String> {
        seal_qualification(artifact, qualification_report_path, seal_path)
            .map_err(|error| format!("{}: {}", error.code, error.message))
    }

    pub fn promote_exact(
        request: ProjectEditorCompositionPromotionRequest,
    ) -> ProjectEditorCompositionPromotionReport {
        promote_exact_with_faults(request, PromotionFaults::default())
    }
}

pub struct ProjectEditorCompositionCacheAdmin;

impl ProjectEditorCompositionCacheAdmin {
    pub fn run(request_path: &Path, report_path: &Path) -> Result<(), String> {
        let request: ProjectEditorCompositionPromotionRequest =
            serde_json::from_slice(&fs::read(request_path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let report = ProjectEditorCompositionArtifact::promote_exact(request);
        atomic_write_json(report_path, &report)
            .map_err(|error| format!("{}: {}", error.code, error.message))
    }
}

fn seal_qualification(
    artifact: &ProjectEditorCompositionArtifact,
    qualification_report_path: &Path,
    seal_path: &Path,
) -> Result<ProjectEditorCompositionQualificationSeal, PromotionError> {
    let descriptor_path = canonical_regular_file(&artifact.descriptor_path, "descriptor")?;
    let build_report_path = canonical_regular_file(&artifact.build_report_path, "build report")?;
    let executable_path = canonical_regular_file(&artifact.executable_path, "executable")?;
    let qualification_report_path =
        canonical_regular_file(qualification_report_path, "qualification report")?;
    let seal_parent = seal_path.parent().ok_or_else(|| {
        PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Qualification seal path must have a parent directory.",
            Some(seal_path),
            "Write the seal beside the qualification report.",
        )
    })?;
    fs::create_dir_all(seal_parent).map_err(|error| io_error("seal", error, seal_parent))?;
    let seal_parent = canonical_regular_directory(seal_parent, "seal parent")?;
    if qualification_report_path.parent() != Some(seal_parent.as_path()) {
        return Err(PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Qualification report and seal must be sibling evidence files.",
            Some(&qualification_report_path),
            "Place the qualification report and seal in the same evidence directory.",
        ));
    }
    let descriptor_bytes = read_regular_bytes(&descriptor_path)?;
    let descriptor: crate::ProjectEditorCompositionDescriptor =
        serde_json::from_slice(&descriptor_bytes)
            .map_err(|error| parse_error("descriptor", error, &descriptor_path))?;
    descriptor.identity.validate().map_err(|error| {
        PromotionError::new(
            &error.code,
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.message,
            Some(&descriptor_path),
            "Rebuild the exact composition artifact.",
        )
    })?;
    let identity_digest = descriptor.identity.digest().map_err(|error| {
        PromotionError::new(
            "project_editor_composition.promotion_identity_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.to_string(),
            Some(&descriptor_path),
            "Regenerate the exact composition identity.",
        )
    })?;
    if descriptor.identity_digest != identity_digest {
        return Err(identity_mismatch(&descriptor_path));
    }
    descriptor.resolved_identity.validate().map_err(|error| {
        PromotionError::new(
            &error.code,
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.message,
            Some(&descriptor_path),
            "Rebuild the exact composition artifact with resolved lineage identity.",
        )
    })?;
    let executable_hash = hash_regular_file(&executable_path)?;
    if descriptor.executable_hash != executable_hash {
        return Err(hash_mismatch(&executable_path));
    }
    let build_report_bytes = read_regular_bytes(&build_report_path)?;
    let build_report: ProjectEditorCompositionBuildReport =
        serde_json::from_slice(&build_report_bytes)
            .map_err(|error| parse_error("build report", error, &build_report_path))?;
    validate_successful_build_report(
        &build_report,
        &descriptor.identity,
        &identity_digest,
        &descriptor.resolved_identity,
    )?;
    let qualification_bytes = read_regular_bytes(&qualification_report_path)?;
    let (qualification_kind, qualification_schema) =
        validate_passed_qualification_report(&qualification_bytes, &identity_digest)?;
    let file_name = qualification_report_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            PromotionError::new(
                "project_editor_composition.promotion_qualification_mismatch",
                ProjectEditorCompositionPromotionStage::ValidateSource,
                "Qualification report file name must be portable UTF-8.",
                Some(&qualification_report_path),
                "Use a portable qualification report file name.",
            )
        })?
        .to_string();
    let seal = ProjectEditorCompositionQualificationSeal {
        schema_version: PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION.to_string(),
        qualification_kind,
        qualification_report_schema_version: qualification_schema,
        qualification_report_file_name: file_name,
        qualification_report_digest: sha256_prefixed(&qualification_bytes),
        composition_identity_digest: identity_digest,
        resolved_identity: descriptor.resolved_identity.clone(),
        executable_hash,
        descriptor_hash: sha256_prefixed(&descriptor_bytes),
        build_report_hash: sha256_prefixed(&build_report_bytes),
        sealed_at: now_epoch_seconds(),
    };
    seal.validate().map_err(|error| {
        PromotionError::new(
            &error.code,
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.message,
            Some(seal_path),
            "Regenerate the qualification seal from passed evidence.",
        )
    })?;
    atomic_write_json(seal_path, &seal).map_err(|error| {
        promotion_from_build_error(
            error,
            ProjectEditorCompositionPromotionStage::ValidateSource,
        )
    })?;
    Ok(seal)
}

fn promote_exact_with_faults(
    request: ProjectEditorCompositionPromotionRequest,
    faults: PromotionFaults,
) -> ProjectEditorCompositionPromotionReport {
    let mut report = empty_report(&request);
    let result = promote_inner(&request, &faults, &mut report);
    if let Err(error) = result {
        report.status = ProjectEditorCompositionPromotionStatus::Failed;
        report.stage = error.stage;
        report.diagnostics.push(ProjectEditorCompositionDiagnostic {
            code: error.code,
            stage: format!("{:?}", error.stage).to_ascii_lowercase(),
            message: error.message,
            path: error.path.map(|path| path.display().to_string()),
            expected_identity: report.expected_identity_digest.clone(),
            actual_identity: report.actual_identity_digest.clone(),
            next_action: error.next_action,
        });
    }
    report
}

fn promote_inner(
    request: &ProjectEditorCompositionPromotionRequest,
    faults: &PromotionFaults,
    report: &mut ProjectEditorCompositionPromotionReport,
) -> Result<(), PromotionError> {
    request.validate().map_err(|error| {
        PromotionError::new(
            &error.code,
            ProjectEditorCompositionPromotionStage::ValidateRequest,
            error.message,
            None,
            "Regenerate the typed promotion request.",
        )
    })?;
    let identity_digest = request.expected_identity.digest().map_err(|error| {
        PromotionError::new(
            "project_editor_composition.promotion_identity_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateRequest,
            error.to_string(),
            None,
            "Regenerate the exact composition identity.",
        )
    })?;
    report.expected_identity_digest = Some(identity_digest.clone());
    let authorized_root = canonical_regular_directory_no_reparse(
        &request.authorized_run_root,
        "authorized run root",
    )?;
    let source_root = canonical_strict_child_directory(
        &request.source_artifact_root,
        &authorized_root,
        "source artifact root",
    )?;
    let seal_path = canonical_strict_child_file(
        &request.qualification_seal_path,
        &authorized_root,
        "qualification seal",
    )?;
    let destination_build_root =
        prepare_plain_directory(&request.destination_cache_root, "destination cache root")?;
    let backup_root =
        prepare_strict_child_directory(&request.backup_root, &authorized_root, "backup root")?;
    if source_root == destination_build_root || source_root == backup_root {
        return Err(path_error(
            "project_editor_composition.promotion_path_invalid",
            &source_root,
            "Promotion source, destination, and backup roots must remain distinct after canonicalization.",
        ));
    }
    report.canonical_source_root = Some(source_root.clone());
    let cache_root = destination_build_root.join(CACHE_ROOT_NAME);
    prepare_plain_directory(&cache_root, "destination owner root")?;
    prepare_plain_directory(&cache_root.join("cache"), "destination cache")?;
    prepare_plain_directory(&cache_root.join("staging"), "destination staging")?;
    prepare_plain_directory(&cache_root.join("reports"), "destination reports")?;
    prepare_plain_directory(&cache_root.join("pins"), "destination pins")?;
    prepare_plain_directory(&cache_root.join("ct"), "destination compilation cache")?;
    let key = request
        .expected_resolved_identity
        .resolved_artifact_key_digest
        .trim_start_matches("sha256:");
    let destination_artifact = cache_root.join("cache").join(key);
    report.canonical_destination_root = Some(destination_artifact.clone());
    report.stage = ProjectEditorCompositionPromotionStage::ValidateDestination;
    if destination_artifact.exists()
        && load_cached_artifact(
            &destination_artifact,
            &request.expected_identity,
            &identity_digest,
            &request.expected_resolved_identity,
        )
        .is_ok()
    {
        let (artifact, _) = load_cached_artifact(
            &destination_artifact,
            &request.expected_identity,
            &identity_digest,
            &request.expected_resolved_identity,
        )
        .map_err(|error| {
            promotion_from_build_error(
                error,
                ProjectEditorCompositionPromotionStage::ValidateDestination,
            )
        })?;
        report.status = ProjectEditorCompositionPromotionStatus::ExactCacheHit;
        report.stage = ProjectEditorCompositionPromotionStage::Complete;
        report.final_executable_hash = Some(artifact.descriptor.executable_hash);
        return Ok(());
    }

    report.stage = ProjectEditorCompositionPromotionStage::ValidateSource;
    let source = validate_qualified_artifact(
        &source_root,
        &seal_path,
        &authorized_root,
        &request.expected_identity,
        &identity_digest,
        &request.expected_resolved_identity,
    )?;
    apply_validated_facts(report, &source, false);
    if faults.mutate_source_after_validation {
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&source.executable_path)
            .and_then(|mut file| file.write_all(b"mutation"))
            .map_err(|error| {
                io_error("source mutation injection", error, &source.executable_path)
            })?;
    }

    let operation_backup_root = backup_root.join(&request.authority_operation_id);
    prepare_plain_directory(&operation_backup_root, "operation backup root")?;
    let backup_path = operation_backup_root.join(key);
    if backup_path.exists() {
        return Err(path_error(
            "project_editor_composition.promotion_publish_failed",
            &backup_path,
            "Operation backup path already exists.",
        ));
    }
    let sequence = PROMOTION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging = cache_root.join("staging").join(format!(
        "promotion-{}-{}-{}",
        request.authority_operation_id,
        std::process::id(),
        sequence
    ));
    if staging.exists() {
        return Err(path_error(
            "project_editor_composition.promotion_publish_failed",
            &staging,
            "Destination promotion staging path already exists.",
        ));
    }
    fs::create_dir_all(&staging).map_err(|error| io_error("staging", error, &staging))?;

    let mut backup = None;
    if destination_artifact.exists() {
        ensure_plain_tree(&destination_artifact)?;
        fs::rename(&destination_artifact, &backup_path)
            .map_err(|error| io_error("backup", error, &destination_artifact))?;
        backup = Some(backup_path.clone());
        report.backup_path = Some(backup_path.clone());
        report.backup_status = ProjectEditorCompositionPromotionBackupStatus::Created;
    }

    let transaction = (|| {
        report.stage = ProjectEditorCompositionPromotionStage::Copy;
        copy_qualified_artifact(&source, &staging)?;
        let copied_seal = staging
            .join("qualification")
            .join("qualification-seal.json");
        let copied = validate_qualified_artifact(
            &staging,
            &copied_seal,
            &staging,
            &request.expected_identity,
            &identity_digest,
            &request.expected_resolved_identity,
        )?;
        apply_validated_facts(report, &copied, true);
        if faults.fail_before_publish {
            return Err(PromotionError::new(
                "project_editor_composition.promotion_publish_failed",
                ProjectEditorCompositionPromotionStage::Publish,
                "Injected publish failure before atomic rename.",
                Some(&staging),
                "Inspect the retained promotion report and retry with fresh staging.",
            ));
        }
        touch_cache_entry(
            &staging,
            &identity_digest,
            &request.expected_identity.project_id,
            directory_size(&staging).map_err(|error| {
                promotion_from_build_error(error, ProjectEditorCompositionPromotionStage::Copy)
            })?,
        )
        .map_err(|error| {
            promotion_from_build_error(error, ProjectEditorCompositionPromotionStage::Copy)
        })?;
        report.stage = ProjectEditorCompositionPromotionStage::Publish;
        publish_directory(&staging, &destination_artifact).map_err(|error| {
            promotion_from_build_error(error, ProjectEditorCompositionPromotionStage::Publish)
        })?;
        if faults.fail_after_publish {
            return Err(PromotionError::new(
                "project_editor_composition.promotion_publish_failed",
                ProjectEditorCompositionPromotionStage::Verify,
                "Injected failure after atomic publish.",
                Some(&destination_artifact),
                "Rollback the destination and inspect the promotion evidence.",
            ));
        }
        report.stage = ProjectEditorCompositionPromotionStage::Verify;
        let (final_artifact, _) = load_cached_artifact(
            &destination_artifact,
            &request.expected_identity,
            &identity_digest,
            &request.expected_resolved_identity,
        )
        .map_err(|error| {
            promotion_from_build_error(error, ProjectEditorCompositionPromotionStage::Verify)
        })?;
        report.final_executable_hash = Some(final_artifact.descriptor.executable_hash);
        Ok(())
    })();

    if let Err(error) = transaction {
        rollback_transaction(
            &cache_root,
            &destination_artifact,
            &staging,
            backup.as_deref(),
            faults.fail_rollback,
            report,
        );
        return Err(error);
    }
    report.backup_status = if backup.is_some() {
        ProjectEditorCompositionPromotionBackupStatus::Retained
    } else {
        ProjectEditorCompositionPromotionBackupStatus::NotRequired
    };
    report.status = ProjectEditorCompositionPromotionStatus::Promoted;
    report.stage = ProjectEditorCompositionPromotionStage::Complete;
    report.cleanup_status = ProjectEditorCompositionPromotionCleanupStatus::Complete;
    Ok(())
}

fn validate_qualified_artifact(
    root: &Path,
    seal_path: &Path,
    evidence_owner: &Path,
    expected: &ProjectEditorCompositionIdentity,
    expected_digest: &str,
    expected_resolved_identity: &crate::ProjectEditorCompositionResolvedIdentity,
) -> Result<ValidatedPromotionArtifact, PromotionError> {
    let root = canonical_regular_directory_no_reparse(root, "artifact root")?;
    let descriptor_path = canonical_strict_child_file(
        &root.join("composition-descriptor.json"),
        &root,
        "composition descriptor",
    )?;
    let build_report_path = canonical_strict_child_file(
        &root.join("build-report.json"),
        &root,
        "composition build report",
    )?;
    let seal_path = canonical_strict_child_file(seal_path, evidence_owner, "qualification seal")?;
    let descriptor_bytes = read_regular_bytes(&descriptor_path)?;
    let descriptor: crate::ProjectEditorCompositionDescriptor =
        serde_json::from_slice(&descriptor_bytes)
            .map_err(|error| parse_error("descriptor", error, &descriptor_path))?;
    if descriptor.identity != *expected
        || descriptor.identity_digest != expected_digest
        || descriptor.resolved_identity != *expected_resolved_identity
    {
        return Err(identity_mismatch(&descriptor_path));
    }
    let actual_digest = descriptor.identity.digest().map_err(|error| {
        PromotionError::new(
            "project_editor_composition.promotion_identity_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.to_string(),
            Some(&descriptor_path),
            "Rebuild the exact composition artifact.",
        )
    })?;
    if actual_digest != expected_digest {
        return Err(identity_mismatch(&descriptor_path));
    }
    let executable_path = canonical_strict_child_file(
        &root.join("bin").join(
            generated_artifact_executable_name(expected).map_err(|error| {
                promotion_from_build_error(
                    error,
                    ProjectEditorCompositionPromotionStage::ValidateSource,
                )
            })?,
        ),
        &root,
        "composition executable",
    )?;
    let executable_hash = hash_regular_file(&executable_path)?;
    if executable_hash != descriptor.executable_hash {
        return Err(hash_mismatch(&executable_path));
    }
    let build_report_bytes = read_regular_bytes(&build_report_path)?;
    let build_report: ProjectEditorCompositionBuildReport =
        serde_json::from_slice(&build_report_bytes)
            .map_err(|error| parse_error("build report", error, &build_report_path))?;
    validate_successful_build_report(
        &build_report,
        expected,
        expected_digest,
        expected_resolved_identity,
    )?;
    let seal_bytes = read_regular_bytes(&seal_path)?;
    let seal: ProjectEditorCompositionQualificationSeal = serde_json::from_slice(&seal_bytes)
        .map_err(|error| parse_error("qualification seal", error, &seal_path))?;
    seal.validate().map_err(|error| {
        PromotionError::new(
            &error.code,
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.message,
            Some(&seal_path),
            "Regenerate qualification evidence from the exact executable.",
        )
    })?;
    if seal.composition_identity_digest != expected_digest
        || seal.resolved_identity != *expected_resolved_identity
        || seal.executable_hash != executable_hash
        || seal.descriptor_hash != sha256_prefixed(&descriptor_bytes)
        || seal.build_report_hash != sha256_prefixed(&build_report_bytes)
    {
        return Err(PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Qualification seal does not bind the current artifact bytes.",
            Some(&seal_path),
            "Re-run qualification for the exact candidate executable.",
        ));
    }
    let qualification_report_path = seal_path
        .parent()
        .unwrap_or(evidence_owner)
        .join(&seal.qualification_report_file_name);
    let qualification_report_path = canonical_strict_child_file(
        &qualification_report_path,
        evidence_owner,
        "qualification report",
    )?;
    let qualification_bytes = read_regular_bytes(&qualification_report_path)?;
    let (kind, schema) =
        validate_passed_qualification_report(&qualification_bytes, expected_digest)?;
    if kind != seal.qualification_kind
        || schema != seal.qualification_report_schema_version
        || sha256_prefixed(&qualification_bytes) != seal.qualification_report_digest
    {
        return Err(PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Qualification report bytes do not match the seal.",
            Some(&qualification_report_path),
            "Restore or regenerate the sealed qualification report.",
        ));
    }
    Ok(ValidatedPromotionArtifact {
        executable_path,
        descriptor_path,
        build_report_path,
        seal_path,
        qualification_report_path,
        executable_hash,
        descriptor_hash: sha256_prefixed(&descriptor_bytes),
        build_report_hash: sha256_prefixed(&build_report_bytes),
        qualification_report_digest: sha256_prefixed(&qualification_bytes),
        identity_digest: actual_digest,
        resolved_identity: descriptor.resolved_identity,
    })
}

fn validate_successful_build_report(
    report: &ProjectEditorCompositionBuildReport,
    expected: &ProjectEditorCompositionIdentity,
    expected_digest: &str,
    expected_resolved_identity: &crate::ProjectEditorCompositionResolvedIdentity,
) -> Result<(), PromotionError> {
    if report.schema_version != PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION
        || report.status != ProjectEditorCompositionBuildStatus::Success
        || report.identity.as_ref() != Some(expected)
        || report.identity_digest.as_deref() != Some(expected_digest)
        || report.resolved_identity.as_ref() != Some(expected_resolved_identity)
    {
        return Err(PromotionError::new(
            "project_editor_composition.promotion_identity_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Composition build report is not a successful exact v2 build for the requested identity.",
            None,
            "Rebuild and qualify the exact composition artifact.",
        ));
    }
    Ok(())
}

fn validate_passed_qualification_report(
    bytes: &[u8],
    expected_digest: &str,
) -> Result<(ProjectEditorCompositionQualificationKind, String), PromotionError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            error.to_string(),
            None,
            "Regenerate the qualification report.",
        )
    })?;
    let schema = value
        .get("schemaVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let kind = match schema {
        "project-editor-composition-qualification-report.v1" => {
            ProjectEditorCompositionQualificationKind::Headless
        }
        "project-editor-composition-real-window-report.v1" => {
            ProjectEditorCompositionQualificationKind::RealWindow
        }
        _ => {
            return Err(PromotionError::new(
                "project_editor_composition.promotion_qualification_mismatch",
                ProjectEditorCompositionPromotionStage::ValidateSource,
                "Qualification report schema is unsupported.",
                None,
                "Use an existing headless or real-window composition qualification producer.",
            ));
        }
    };
    if value.get("status").and_then(Value::as_str) != Some("passed")
        || value
            .get("compositionIdentityDigest")
            .and_then(Value::as_str)
            != Some(expected_digest)
    {
        return Err(PromotionError::new(
            "project_editor_composition.promotion_qualification_mismatch",
            ProjectEditorCompositionPromotionStage::ValidateSource,
            "Qualification report did not pass for the exact composition identity.",
            None,
            "Re-run qualification for the exact candidate.",
        ));
    }
    Ok((kind, schema.to_string()))
}

fn copy_qualified_artifact(
    source: &ValidatedPromotionArtifact,
    staging: &Path,
) -> Result<(), PromotionError> {
    let executable_name = source.executable_path.file_name().ok_or_else(|| {
        path_error(
            "project_editor_composition.promotion_copy_verification_failed",
            &source.executable_path,
            "Source executable has no file name.",
        )
    })?;
    fs::create_dir_all(staging.join("bin"))
        .and_then(|_| fs::create_dir_all(staging.join("qualification")))
        .map_err(|error| io_error("copy", error, staging))?;
    copy_regular_file(
        &source.executable_path,
        &staging.join("bin").join(executable_name),
    )?;
    copy_regular_file(
        &source.descriptor_path,
        &staging.join("composition-descriptor.json"),
    )?;
    copy_regular_file(
        &source.build_report_path,
        &staging.join("build-report.json"),
    )?;
    copy_regular_file(
        &source.seal_path,
        &staging
            .join("qualification")
            .join("qualification-seal.json"),
    )?;
    let report_name = source
        .qualification_report_path
        .file_name()
        .ok_or_else(|| {
            path_error(
                "project_editor_composition.promotion_copy_verification_failed",
                &source.qualification_report_path,
                "Qualification report has no file name.",
            )
        })?;
    copy_regular_file(
        &source.qualification_report_path,
        &staging.join("qualification").join(report_name),
    )
}

fn rollback_transaction(
    cache_root: &Path,
    destination: &Path,
    staging: &Path,
    backup: Option<&Path>,
    fail_rollback: bool,
    report: &mut ProjectEditorCompositionPromotionReport,
) {
    report.stage = ProjectEditorCompositionPromotionStage::Rollback;
    if fail_rollback {
        report.rollback_status = ProjectEditorCompositionPromotionRollbackStatus::Failed;
        report.cleanup_status = ProjectEditorCompositionPromotionCleanupStatus::Retained;
        for path in [Some(destination), Some(staging), backup]
            .into_iter()
            .flatten()
        {
            if path.exists() {
                report.retained_paths.push(path.to_path_buf());
            }
        }
        return;
    }
    let mut failed = false;
    if destination.exists() && remove_owned_cache_entry(cache_root, destination).is_err() {
        failed = true;
        report.retained_paths.push(destination.to_path_buf());
    }
    if let Some(backup) = backup {
        if !failed && fs::rename(backup, destination).is_ok() {
            report.backup_status = ProjectEditorCompositionPromotionBackupStatus::Restored;
        } else {
            failed = true;
            if backup.exists() {
                report.retained_paths.push(backup.to_path_buf());
            }
        }
    }
    if staging.exists() && remove_owned_promotion_staging(cache_root, staging).is_err() {
        failed = true;
        report.retained_paths.push(staging.to_path_buf());
    }
    report.rollback_status = if failed {
        ProjectEditorCompositionPromotionRollbackStatus::Failed
    } else {
        ProjectEditorCompositionPromotionRollbackStatus::Succeeded
    };
    report.cleanup_status = if failed {
        ProjectEditorCompositionPromotionCleanupStatus::Retained
    } else {
        ProjectEditorCompositionPromotionCleanupStatus::Complete
    };
}

fn remove_owned_promotion_staging(cache_root: &Path, staging: &Path) -> Result<(), PromotionError> {
    let staging_owner = canonical_regular_directory_no_reparse(
        &cache_root.join("staging"),
        "destination staging owner",
    )?;
    let canonical_staging = canonical_regular_directory_no_reparse(staging, "promotion staging")?;
    let has_owned_name = canonical_staging
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("promotion-"));
    if canonical_staging.parent() != Some(staging_owner.as_path()) || !has_owned_name {
        return Err(path_error(
            "project_editor_composition.promotion_cleanup_scope_rejected",
            staging,
            "Promotion cleanup target must be a regular direct child of the destination staging owner.",
        ));
    }
    fs::remove_dir_all(&canonical_staging)
        .map_err(|error| io_error("promotion staging cleanup", error, &canonical_staging))
}

fn apply_validated_facts(
    report: &mut ProjectEditorCompositionPromotionReport,
    artifact: &ValidatedPromotionArtifact,
    copied: bool,
) {
    report.actual_identity_digest = Some(artifact.identity_digest.clone());
    report.actual_resolved_identity = Some(artifact.resolved_identity.clone());
    if copied {
        report.copied_executable_hash = Some(artifact.executable_hash.clone());
    } else {
        report.source_executable_hash = Some(artifact.executable_hash.clone());
    }
    report.descriptor_hash = Some(artifact.descriptor_hash.clone());
    report.build_report_hash = Some(artifact.build_report_hash.clone());
    report.qualification_evidence_digest = Some(artifact.qualification_report_digest.clone());
}

fn empty_report(
    request: &ProjectEditorCompositionPromotionRequest,
) -> ProjectEditorCompositionPromotionReport {
    ProjectEditorCompositionPromotionReport {
        schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION.to_string(),
        status: ProjectEditorCompositionPromotionStatus::Failed,
        stage: ProjectEditorCompositionPromotionStage::ValidateRequest,
        authority_operation_id: request.authority_operation_id.clone(),
        canonical_source_root: None,
        canonical_destination_root: None,
        backup_path: None,
        expected_identity_digest: None,
        actual_identity_digest: None,
        expected_resolved_identity: Some(request.expected_resolved_identity.clone()),
        actual_resolved_identity: None,
        source_executable_hash: None,
        copied_executable_hash: None,
        final_executable_hash: None,
        descriptor_hash: None,
        build_report_hash: None,
        qualification_evidence_digest: None,
        backup_status: ProjectEditorCompositionPromotionBackupStatus::NotRequired,
        rollback_status: ProjectEditorCompositionPromotionRollbackStatus::NotRequired,
        cleanup_status: ProjectEditorCompositionPromotionCleanupStatus::Complete,
        retained_paths: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn copy_regular_file(source: &Path, destination: &Path) -> Result<(), PromotionError> {
    canonical_regular_file(source, "promotion source file")?;
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| io_error("copy", error, destination))
}

fn canonical_regular_directory(path: &Path, role: &str) -> Result<PathBuf, PromotionError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| io_error(role, error, path))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(role, error, path))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(path_error(
            "project_editor_composition.promotion_source_untrusted_path",
            path,
            format!("{role} must be a regular directory without links or reparse points."),
        ));
    }
    Ok(canonical)
}

fn canonical_regular_directory_no_reparse(
    path: &Path,
    role: &str,
) -> Result<PathBuf, PromotionError> {
    ensure_plain_tree(path)?;
    canonical_regular_directory(path, role)
}

fn canonical_regular_file(path: &Path, role: &str) -> Result<PathBuf, PromotionError> {
    ensure_plain_tree(path)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| io_error(role, error, path))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(role, error, path))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(path_error(
            "project_editor_composition.promotion_source_untrusted_path",
            path,
            format!("{role} must be a regular file without links or reparse points."),
        ));
    }
    Ok(canonical)
}

fn canonical_strict_child_directory(
    path: &Path,
    owner: &Path,
    role: &str,
) -> Result<PathBuf, PromotionError> {
    let canonical = canonical_regular_directory_no_reparse(path, role)?;
    if canonical == owner || !canonical.starts_with(owner) {
        return Err(path_error(
            "project_editor_composition.promotion_source_outside_authorized_root",
            path,
            format!("{role} must be a strict child of the authorized root."),
        ));
    }
    Ok(canonical)
}

fn canonical_strict_child_file(
    path: &Path,
    owner: &Path,
    role: &str,
) -> Result<PathBuf, PromotionError> {
    let canonical = canonical_regular_file(path, role)?;
    if !canonical.starts_with(owner) {
        return Err(path_error(
            "project_editor_composition.promotion_source_outside_authorized_root",
            path,
            format!("{role} must be contained by its authorized owner."),
        ));
    }
    Ok(canonical)
}

fn prepare_plain_directory(path: &Path, role: &str) -> Result<PathBuf, PromotionError> {
    fs::create_dir_all(path).map_err(|error| io_error(role, error, path))?;
    canonical_regular_directory_no_reparse(path, role)
}

fn prepare_strict_child_directory(
    path: &Path,
    owner: &Path,
    role: &str,
) -> Result<PathBuf, PromotionError> {
    let canonical = prepare_plain_directory(path, role)?;
    if canonical == owner || !canonical.starts_with(owner) {
        return Err(path_error(
            "project_editor_composition.promotion_source_outside_authorized_root",
            path,
            format!("{role} must be a strict child of the authorized run root."),
        ));
    }
    Ok(canonical)
}

fn ensure_plain_tree(path: &Path) -> Result<(), PromotionError> {
    let mut cursor = Some(path);
    while let Some(current) = cursor {
        if current.exists() {
            let metadata = fs::symlink_metadata(current)
                .map_err(|error| io_error("path inspection", error, current))?;
            if is_link_or_reparse(&metadata) {
                return Err(path_error(
                    "project_editor_composition.promotion_source_untrusted_path",
                    current,
                    "Promotion paths cannot traverse links, junctions, or reparse points.",
                ));
            }
        }
        cursor = current.parent();
    }
    Ok(())
}

fn read_regular_bytes(path: &Path) -> Result<Vec<u8>, PromotionError> {
    canonical_regular_file(path, "evidence file")?;
    fs::read(path).map_err(|error| io_error("read", error, path))
}

fn hash_regular_file(path: &Path) -> Result<String, PromotionError> {
    read_regular_bytes(path).map(|bytes| sha256_prefixed(&bytes))
}

fn parse_error(role: &str, error: serde_json::Error, path: &Path) -> PromotionError {
    PromotionError::new(
        "project_editor_composition.promotion_qualification_mismatch",
        ProjectEditorCompositionPromotionStage::ValidateSource,
        format!("Invalid {role}: {error}"),
        Some(path),
        "Regenerate the typed evidence from the exact candidate.",
    )
}

fn identity_mismatch(path: &Path) -> PromotionError {
    PromotionError::new(
        "project_editor_composition.promotion_identity_mismatch",
        ProjectEditorCompositionPromotionStage::ValidateSource,
        "Composition artifact identity is not the exact requested identity.",
        Some(path),
        "Use a candidate built for the exact destination identity.",
    )
}

fn hash_mismatch(path: &Path) -> PromotionError {
    PromotionError::new(
        "project_editor_composition.promotion_executable_hash_mismatch",
        ProjectEditorCompositionPromotionStage::ValidateSource,
        "Composition executable bytes do not match the sealed descriptor.",
        Some(path),
        "Rebuild and qualify the exact candidate executable.",
    )
}

fn path_error(code: &str, path: &Path, message: impl Into<String>) -> PromotionError {
    PromotionError::new(
        code,
        ProjectEditorCompositionPromotionStage::ValidateRequest,
        message,
        Some(path),
        "Use explicit contained regular paths and retry.",
    )
}

fn io_error(role: &str, error: std::io::Error, path: &Path) -> PromotionError {
    PromotionError::new(
        "project_editor_composition.promotion_publish_failed",
        ProjectEditorCompositionPromotionStage::Publish,
        format!("{role} failed: {error}"),
        Some(path),
        "Inspect the promotion evidence and filesystem state before retrying.",
    )
}

fn promotion_from_build_error(
    error: crate::project_editor_composition_artifact::CompositionBuildError,
    stage: ProjectEditorCompositionPromotionStage,
) -> PromotionError {
    PromotionError::new(
        &error.code,
        stage,
        error.message,
        error.path.as_deref().map(Path::new),
        error.next_action,
    )
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_editor_composition_artifact::read_json;
    use crate::{
        GeneratedCompositionLockLineage, ProjectEditorCompositionBuildSourceKind,
        ProjectEditorCompositionCacheStatus, ProjectEditorCompositionCompilationCacheAffinity,
        ProjectEditorCompositionDescriptor, ProjectEditorCompositionProcessPriority,
        ProjectEditorCompositionResolvedIdentity,
        GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;
    use std::io::Write;

    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        destination: PathBuf,
        evidence: PathBuf,
        identity: ProjectEditorCompositionIdentity,
        request: ProjectEditorCompositionPromotionRequest,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if std::env::var_os("AIFE_282_C2_RUN_ROOT").is_none() {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn new_fixture(label: &str) -> Fixture {
        let owner = std::env::var_os("AIFE_282_C2_RUN_ROOT")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("matrix"))
            .unwrap_or_else(std::env::temp_dir);
        let root = owner.join(format!(
            "aife-282-c2-{label}-{}-{}",
            std::process::id(),
            PROMOTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source-qualified").join("artifact");
        let destination = root.join("destination-cache");
        let backup = root.join("backup");
        let evidence = root.join("evidence");
        fs::create_dir_all(source.join("bin")).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::create_dir_all(&backup).unwrap();
        fs::create_dir_all(&evidence).unwrap();
        let identity = identity("fixture.project");
        let resolved_identity = resolved_identity(&identity);
        write_artifact(&source, &evidence, &identity, &resolved_identity);
        let request = ProjectEditorCompositionPromotionRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION.to_string(),
            authority_operation_id: format!("op-{label}"),
            authorized_run_root: root.clone(),
            source_artifact_root: source.clone(),
            destination_cache_root: destination.clone(),
            backup_root: backup.clone(),
            qualification_seal_path: evidence.join("qualification-seal.json"),
            expected_identity: identity.clone(),
            expected_resolved_identity: resolved_identity,
        };
        Fixture {
            root,
            source,
            destination,
            evidence,
            identity,
            request,
        }
    }

    fn identity(project_id: &str) -> ProjectEditorCompositionIdentity {
        ProjectEditorCompositionIdentity {
            schema_version: crate::PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: project_id.to_string(),
            module_id: "fixture.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc-fixture".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
            normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
            dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
        }
    }

    fn resolved_identity(
        identity: &ProjectEditorCompositionIdentity,
    ) -> ProjectEditorCompositionResolvedIdentity {
        ProjectEditorCompositionResolvedIdentity::new(
            identity.digest().unwrap(),
            &GeneratedCompositionLockLineage {
                schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
                lock_input_digest: format!("sha256:{}", "1".repeat(64)),
                raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
                resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
            },
        )
        .unwrap()
    }

    fn write_artifact(
        root: &Path,
        evidence: &Path,
        identity: &ProjectEditorCompositionIdentity,
        resolved_identity: &ProjectEditorCompositionResolvedIdentity,
    ) {
        let digest = identity.digest().unwrap();
        let executable = root
            .join("bin")
            .join(generated_artifact_executable_name(identity).unwrap());
        fs::write(&executable, b"qualified-editor").unwrap();
        let descriptor = ProjectEditorCompositionDescriptor {
            schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity.clone(),
            identity_digest: digest.clone(),
            resolved_identity: resolved_identity.clone(),
            executable_hash: sha256_prefixed(b"qualified-editor"),
            created_at: 1,
        };
        atomic_write_json(&root.join("composition-descriptor.json"), &descriptor).unwrap();
        let report = ProjectEditorCompositionBuildReport {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionBuildStatus::Success,
            identity: Some(identity.clone()),
            identity_digest: Some(digest.clone()),
            resolved_identity: Some(resolved_identity.clone()),
            artifact: None,
            source_kind: ProjectEditorCompositionBuildSourceKind::ControlledBuild,
            cache_status: ProjectEditorCompositionCacheStatus::Rebuilt,
            cleanup_status: "staging_published".to_string(),
            artifact_size_bytes: Some(1),
            steps: Vec::new(),
            deadline_policy: None,
            qos_policy: None,
            system_facts: None,
            qos_decision: None,
            requested_priority: ProjectEditorCompositionProcessPriority::BelowNormal,
            effective_priority: Some(ProjectEditorCompositionProcessPriority::BelowNormal),
            priority_applied: true,
            cancellation_requested: false,
            process_tree_terminated: false,
            output_readers_joined: true,
            root_wait_completed: true,
            process_group_released: true,
            owned_process_cleanup_confirmed: true,
            release_build_soft_budget_exceeded: false,
            release_build_soft_budget_exceeded_at_ms: None,
            compilation_cache_compatibility_digest: Some(format!("sha256:{}", "4".repeat(64))),
            compilation_cache_affinity: ProjectEditorCompositionCompilationCacheAffinity::Cold,
            canonical_target_anchor_digest: Some(format!("sha256:{}", "5".repeat(64))),
            canonical_target_root_digest: Some(format!("sha256:{}", "6".repeat(64))),
            cross_root_portable: false,
            worker_joined: false,
            redraw_policy_hz: Some(10),
            stage_durations_ms: BTreeMap::new(),
            diagnostics: Vec::new(),
        };
        atomic_write_json(&root.join("build-report.json"), &report).unwrap();
        let qualification = serde_json::json!({
            "schemaVersion": "project-editor-composition-qualification-report.v1",
            "status": "passed",
            "compositionIdentityDigest": digest,
            "projectId": identity.project_id,
            "moduleId": identity.module_id
        });
        atomic_write_json(&evidence.join("qualification-report.json"), &qualification).unwrap();
        let artifact = ProjectEditorCompositionArtifact {
            schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
            executable_path: executable,
            descriptor_path: root.join("composition-descriptor.json"),
            build_report_path: root.join("build-report.json"),
            descriptor,
        };
        ProjectEditorCompositionArtifact::seal_qualification(
            &artifact,
            &evidence.join("qualification-report.json"),
            &evidence.join("qualification-seal.json"),
        )
        .unwrap();
    }

    #[test]
    fn project_editor_composition_promotion_schema_round_trips_and_rejects_unknown_fields() {
        let fixture = new_fixture("schema");
        fixture.request.validate().unwrap();
        let value = serde_json::to_value(&fixture.request).unwrap();
        assert_eq!(
            serde_json::from_value::<ProjectEditorCompositionPromotionRequest>(value.clone())
                .unwrap(),
            fixture.request
        );
        let mut unknown = value;
        unknown
            .as_object_mut()
            .unwrap()
            .insert("searchLatest".to_string(), Value::Bool(true));
        assert!(
            serde_json::from_value::<ProjectEditorCompositionPromotionRequest>(unknown).is_err()
        );
    }

    #[test]
    fn project_editor_composition_promotion_exact_happy_path_and_noop_hit() {
        let fixture = new_fixture("happy");
        let first = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
        persist_report("exact-promotion", &first);
        assert_eq!(
            first.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );
        assert_eq!(first.source_executable_hash, first.copied_executable_hash);
        assert_eq!(first.copied_executable_hash, first.final_executable_hash);
        let second = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
        persist_report("exact-promotion-noop", &second);
        assert_eq!(
            second.status,
            ProjectEditorCompositionPromotionStatus::ExactCacheHit
        );
    }

    #[test]
    fn project_editor_composition_promotion_rejects_identity_hash_report_seal_and_path_tamper() {
        let mut fixture = new_fixture("negative");
        fixture.request.expected_identity = identity("other.project");
        fixture.request.expected_resolved_identity =
            resolved_identity(&fixture.request.expected_identity);
        let report = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
        persist_report("identity-mismatch", &report);
        assert_eq!(
            report.status,
            ProjectEditorCompositionPromotionStatus::Failed
        );
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_identity_mismatch"
        }));

        let fixture = new_fixture("exe-tamper");
        fs::write(
            fixture
                .source
                .join("bin")
                .join(generated_artifact_executable_name(&fixture.identity).unwrap()),
            b"tampered",
        )
        .unwrap();
        let report = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
        persist_report("executable-tamper", &report);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_executable_hash_mismatch"
        }));

        for (label, path) in [
            ("descriptor", "composition-descriptor.json"),
            ("report", "build-report.json"),
        ] {
            let fixture = new_fixture(label);
            fs::OpenOptions::new()
                .append(true)
                .open(fixture.source.join(path))
                .unwrap()
                .write_all(b" ")
                .unwrap();
            let report = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
            persist_report(&format!("{label}-tamper"), &report);
            assert_eq!(
                report.status,
                ProjectEditorCompositionPromotionStatus::Failed
            );
        }
        let fixture = new_fixture("seal-tamper");
        fs::OpenOptions::new()
            .append(true)
            .open(fixture.evidence.join("qualification-report.json"))
            .unwrap()
            .write_all(b" ")
            .unwrap();
        let report = ProjectEditorCompositionArtifact::promote_exact(fixture.request.clone());
        persist_report("seal-tamper", &report);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_qualification_mismatch"
        }));

        let fixture = new_fixture("outside");
        let mut request = fixture.request.clone();
        request.source_artifact_root = std::env::temp_dir();
        let report = ProjectEditorCompositionArtifact::promote_exact(request);
        persist_report("source-outside-root", &report);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_source_outside_authorized_root"
        }));
    }

    #[test]
    fn project_editor_composition_promotion_detects_copy_mutation_and_rolls_back_publish_failure() {
        let fixture = new_fixture("copy-mutation");
        let report = promote_exact_with_faults(
            fixture.request.clone(),
            PromotionFaults {
                mutate_source_after_validation: true,
                ..PromotionFaults::default()
            },
        );
        persist_report("copy-after-verify-mutation", &report);
        assert_eq!(
            report.status,
            ProjectEditorCompositionPromotionStatus::Failed
        );
        assert_ne!(report.source_executable_hash, report.copied_executable_hash);

        let fixture = new_fixture("rollback");
        let identity_digest = fixture.identity.digest().unwrap();
        let destination_artifact = fixture
            .destination
            .join(CACHE_ROOT_NAME)
            .join("cache")
            .join(identity_digest.trim_start_matches("sha256:"));
        fs::create_dir_all(&destination_artifact).unwrap();
        fs::write(destination_artifact.join("old.bin"), b"old-bytes").unwrap();
        let report = promote_exact_with_faults(
            fixture.request.clone(),
            PromotionFaults {
                fail_before_publish: true,
                ..PromotionFaults::default()
            },
        );
        persist_report("publish-failure-rollback", &report);
        assert_eq!(
            report.status,
            ProjectEditorCompositionPromotionStatus::Failed
        );
        assert_eq!(
            report.rollback_status,
            ProjectEditorCompositionPromotionRollbackStatus::Succeeded
        );
        assert_eq!(
            fs::read(destination_artifact.join("old.bin")).unwrap(),
            b"old-bytes"
        );
    }

    #[test]
    fn project_editor_composition_promotion_rollback_failure_is_terminal_and_retained() {
        let fixture = new_fixture("rollback-failed");
        let identity_digest = fixture.identity.digest().unwrap();
        let destination_artifact = fixture
            .destination
            .join(CACHE_ROOT_NAME)
            .join("cache")
            .join(identity_digest.trim_start_matches("sha256:"));
        fs::create_dir_all(&destination_artifact).unwrap();
        fs::write(destination_artifact.join("old.bin"), b"old-bytes").unwrap();
        let report = promote_exact_with_faults(
            fixture.request.clone(),
            PromotionFaults {
                fail_after_publish: true,
                fail_rollback: true,
                ..PromotionFaults::default()
            },
        );
        persist_report("rollback-failure-retained", &report);
        assert_eq!(
            report.status,
            ProjectEditorCompositionPromotionStatus::Failed
        );
        assert_eq!(
            report.rollback_status,
            ProjectEditorCompositionPromotionRollbackStatus::Failed
        );
        assert_eq!(
            report.cleanup_status,
            ProjectEditorCompositionPromotionCleanupStatus::Retained
        );
        assert!(!report.retained_paths.is_empty());
    }

    #[test]
    fn project_editor_composition_cache_admin_is_typed_and_writes_report() {
        let fixture = new_fixture("admin");
        let request_path = fixture.root.join("request.json");
        let report_path = fixture.root.join("admin-report.json");
        atomic_write_json(&request_path, &fixture.request).unwrap();
        ProjectEditorCompositionCacheAdmin::run(&request_path, &report_path).unwrap();
        let report: ProjectEditorCompositionPromotionReport = read_json(&report_path).unwrap();
        assert_eq!(
            report.status,
            ProjectEditorCompositionPromotionStatus::Promoted
        );
    }

    #[cfg(windows)]
    #[test]
    fn project_editor_composition_promotion_rejects_source_and_destination_reparse_roots() {
        let fixture = new_fixture("source-reparse");
        let source_link = fixture.root.join("source-reparse-link");
        create_junction(&source_link, &fixture.source);
        let mut request = fixture.request.clone();
        request.source_artifact_root = source_link;
        let source_report = ProjectEditorCompositionArtifact::promote_exact(request);
        persist_report("source-reparse", &source_report);
        assert!(source_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_source_untrusted_path"
        }));

        let fixture = new_fixture("destination-reparse");
        let destination_target = fixture.root.join("destination-reparse-target");
        fs::create_dir_all(&destination_target).unwrap();
        let destination_link = fixture.root.join("destination-reparse-link");
        create_junction(&destination_link, &destination_target);
        let mut request = fixture.request.clone();
        request.destination_cache_root = destination_link;
        let destination_report = ProjectEditorCompositionArtifact::promote_exact(request);
        persist_report("destination-reparse", &destination_report);
        assert!(destination_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_editor_composition.promotion_source_untrusted_path"
        }));
    }

    #[cfg(windows)]
    fn create_junction(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn persist_report(label: &str, report: &ProjectEditorCompositionPromotionReport) {
        let Some(evidence_root) = std::env::var_os("AIFE_282_C2_EVIDENCE_ROOT") else {
            return;
        };
        let evidence_root = PathBuf::from(evidence_root);
        assert!(evidence_root.is_absolute());
        fs::create_dir_all(&evidence_root).unwrap();
        atomic_write_json(&evidence_root.join(format!("matrix-{label}.json")), report).unwrap();
    }
}
