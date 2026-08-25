use crate::project_runtime_player_staging::ProjectRuntimeProductionStaging;
use crate::{
    generated_composition_lock_lineage, resolve_project_editor_composition_build_qos,
    GeneratedCompositionLockInput, GeneratedCompositionLockLineage,
    ProjectEditorCompositionArtifact, ProjectEditorCompositionBuildReport,
    ProjectEditorCompositionBuildRequest, ProjectEditorCompositionBuildSourceKind,
    ProjectEditorCompositionBuildStatus, ProjectEditorCompositionBuildStep,
    ProjectEditorCompositionCacheStatus, ProjectEditorCompositionCompilationCacheAffinity,
    ProjectEditorCompositionDescriptor, ProjectEditorCompositionDiagnostic,
    ProjectEditorCompositionIdentity, ProjectEditorCompositionPreparationControl,
    ProjectEditorCompositionPreparationPhase, ProjectEditorCompositionProcessPriority,
    ProjectEditorCompositionResolvedIdentity, ProjectEditorCompositionSystemFacts,
    GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use runtime_cli::{
    run_bounded_child_process_cancellable, BoundedChildProcessExitReason,
    BoundedChildProcessPriority, BoundedChildProcessRequest, BoundedChildProcessResult,
};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const CACHE_ROOT_NAME: &str = "project-editor-compositions";
const COMPILATION_CACHE_SCHEMA_VERSION: &str = "project-editor-composition-compilation-cache.v2";
static BUILD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheEntryMetadata {
    schema_version: String,
    identity_digest: String,
    project_id: String,
    size_bytes: u64,
    last_used_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompilationCacheAffinityMarker {
    schema_version: String,
    compatibility_digest: String,
    canonical_target_anchor_digest: String,
    canonical_target_root_digest: String,
    creator_identity: String,
}

#[derive(Debug)]
pub(crate) struct CompositionBuildError {
    pub(crate) code: String,
    pub(crate) stage: String,
    pub(crate) message: String,
    pub(crate) path: Option<String>,
    pub(crate) next_action: String,
}

impl CompositionBuildError {
    fn new(
        code: &str,
        stage: &str,
        message: impl Into<String>,
        path: Option<&Path>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            stage: stage.to_string(),
            message: message.into(),
            path: path.map(|value| value.display().to_string()),
            next_action: next_action.into(),
        }
    }
}

impl ProjectEditorCompositionArtifact {
    pub fn prepare(
        request: ProjectEditorCompositionBuildRequest,
        control: ProjectEditorCompositionPreparationControl,
    ) -> ProjectEditorCompositionBuildReport {
        Self::prepare_with_progress(request, control, &mut |_| {})
    }

    pub fn prepare_with_progress(
        request: ProjectEditorCompositionBuildRequest,
        control: ProjectEditorCompositionPreparationControl,
        progress: &mut dyn FnMut(ProjectEditorCompositionPreparationPhase),
    ) -> ProjectEditorCompositionBuildReport {
        let mut report = ProjectEditorCompositionBuildReport {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionBuildStatus::Failed,
            identity: Some(request.expected_identity.clone()),
            identity_digest: None,
            resolved_identity: None,
            artifact: None,
            source_kind: ProjectEditorCompositionBuildSourceKind::NotDetermined,
            cache_status: ProjectEditorCompositionCacheStatus::NotChecked,
            cleanup_status: "not_required".to_string(),
            artifact_size_bytes: None,
            steps: Vec::new(),
            deadline_policy: Some(request.deadline_policy.clone()),
            qos_policy: Some(request.qos_policy.clone()),
            system_facts: None,
            qos_decision: None,
            requested_priority: ProjectEditorCompositionProcessPriority::BelowNormal,
            effective_priority: None,
            priority_applied: false,
            cancellation_requested: false,
            process_tree_terminated: false,
            output_readers_joined: false,
            root_wait_completed: false,
            process_group_released: false,
            owned_process_cleanup_confirmed: false,
            release_build_soft_budget_exceeded: false,
            release_build_soft_budget_exceeded_at_ms: None,
            compilation_cache_compatibility_digest: None,
            compilation_cache_affinity: ProjectEditorCompositionCompilationCacheAffinity::Cold,
            canonical_target_anchor_digest: None,
            canonical_target_root_digest: None,
            cross_root_portable: false,
            worker_joined: false,
            redraw_policy_hz: Some(10),
            stage_durations_ms: Default::default(),
            diagnostics: Vec::new(),
        };
        let mut staging_root = None;
        if let Err(error) =
            prepare_inner(&request, &control, progress, &mut report, &mut staging_root)
        {
            if error.code == "project_editor_composition.cancelled" {
                progress(ProjectEditorCompositionPreparationPhase::Cancelled);
            }
            report.diagnostics.push(ProjectEditorCompositionDiagnostic {
                code: error.code,
                stage: error.stage,
                message: error.message,
                path: error.path,
                expected_identity: report.identity_digest.clone(),
                actual_identity: None,
                next_action: error.next_action,
            });
            if let Some(staging) = staging_root {
                report.cleanup_status = cleanup_owned_directory(&request.build_root, &staging)
                    .unwrap_or_else(|status| status);
            }
            if failure_report_is_allowed(&request.project_root, &request.build_root) {
                let _ = write_failure_report(&request.build_root, &report);
            }
        }
        report
    }
}

fn prepare_inner(
    request: &ProjectEditorCompositionBuildRequest,
    control: &ProjectEditorCompositionPreparationControl,
    progress: &mut dyn FnMut(ProjectEditorCompositionPreparationPhase),
    report: &mut ProjectEditorCompositionBuildReport,
    staging_slot: &mut Option<PathBuf>,
) -> Result<(), CompositionBuildError> {
    request.validate().map_err(|error| {
        CompositionBuildError::new(
            &error.code,
            "validate_request",
            error.message,
            None,
            "Regenerate the composition request from current project and Engine identities.",
        )
    })?;
    progress(ProjectEditorCompositionPreparationPhase::Inspecting);
    let system_facts = ProjectEditorCompositionSystemFacts::collect();
    let qos_decision =
        resolve_project_editor_composition_build_qos(&request.qos_policy, system_facts).map_err(
            |error| {
                CompositionBuildError::new(
                    &error.code,
                    "resolve_build_qos",
                    error.message,
                    None,
                    "Fix the composition Build QoS policy and retry.",
                )
            },
        )?;
    report.system_facts = Some(system_facts);
    report.qos_decision = Some(qos_decision);
    let project_root = canonical_regular_directory(&request.project_root, "project root")?;
    let sdk_root = canonical_regular_directory(&request.engine_sdk_root, "Engine SDK root")?;
    let build_root = prepare_build_root(&request.build_root, &project_root)?;
    let identity_digest = request.expected_identity.digest().map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.identity_digest_failed",
            "validate_request",
            error.to_string(),
            None,
            "Regenerate the composition identity.",
        )
    })?;
    report.identity_digest = Some(identity_digest.clone());

    let plan =
        ProjectRuntimeProductionStaging::plan(&project_root, &sdk_root).map_err(|error| {
            CompositionBuildError::new(
                &error.code,
                "production_staging_plan",
                error.message,
                Some(&project_root),
                "Fix the ProjectRust manifest or dependency policy and retry.",
            )
        })?;
    validate_staging_identity(&request.expected_identity, &plan)?;

    let cache_root = build_root.join(CACHE_ROOT_NAME);
    ensure_owned_directory(&cache_root)?;
    ensure_owned_directory(&cache_root.join("cache"))?;
    ensure_owned_directory(&cache_root.join("staging"))?;
    ensure_owned_directory(&cache_root.join("reports"))?;
    ensure_owned_directory(&cache_root.join("pins"))?;
    ensure_owned_directory(&cache_root.join("ct"))?;
    report.source_kind = ProjectEditorCompositionBuildSourceKind::ControlledBuild;
    report.cache_status = ProjectEditorCompositionCacheStatus::Miss;

    let requested_key = identity_digest.trim_start_matches("sha256:");
    let sequence = BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    progress(ProjectEditorCompositionPreparationPhase::Staging);
    let staging_root = cache_root.join("staging").join(format!(
        "{}-{}-{}",
        &requested_key[..16],
        std::process::id(),
        sequence
    ));
    ensure_owned_directory(&staging_root)?;
    *staging_slot = Some(staging_root.clone());
    ProjectRuntimeProductionStaging::stage(&project_root, &staging_root, &plan).map_err(
        |error| {
            CompositionBuildError::new(
                &error.code,
                "production_staging_copy",
                error.message,
                Some(&staging_root),
                "Inspect the controlled project source tree and retry.",
            )
        },
    )?;
    write_generated_composition(
        &staging_root,
        &sdk_root,
        &plan.manifest.runtime_module.cargo_package,
        &request.expected_identity,
    )?;

    let cargo = request
        .cargo_executable
        .clone()
        .or_else(|| std::env::var_os("CARGO").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let generated_root = staging_root.join("GeneratedEditor");
    progress(ProjectEditorCompositionPreparationPhase::Compiling);
    let lock_input = generated_lock_input(request, &plan, &generated_root)?;
    let lock_input_digest = lock_input.digest().map_err(contract_build_error)?;
    let lineage = prepare_generated_lock_lineage(
        &cache_root,
        &generated_root,
        &cargo,
        request,
        control,
        report,
        lock_input_digest,
    )?;
    let resolved_identity =
        ProjectEditorCompositionResolvedIdentity::new(identity_digest.clone(), &lineage)
            .map_err(contract_build_error)?;
    report.resolved_identity = Some(resolved_identity.clone());
    let key = resolved_identity
        .resolved_artifact_key_digest
        .trim_start_matches("sha256:");
    let artifact_root = cache_root.join("cache").join(key);
    progress(ProjectEditorCompositionPreparationPhase::CacheLookup);
    if artifact_root.exists() {
        match load_cached_artifact(
            &artifact_root,
            &request.expected_identity,
            &identity_digest,
            &resolved_identity,
        ) {
            Ok((artifact, size)) => {
                touch_cache_entry(
                    &artifact_root,
                    &identity_digest,
                    &request.expected_identity.project_id,
                    size,
                )?;
                let _ = cleanup_owned_directory(&cache_root, &staging_root);
                *staging_slot = None;
                report.status = ProjectEditorCompositionBuildStatus::Success;
                report.source_kind = ProjectEditorCompositionBuildSourceKind::ExactCache;
                report.cache_status = ProjectEditorCompositionCacheStatus::Hit;
                report.cleanup_status = "cache_reused".to_string();
                report.artifact_size_bytes = Some(size);
                report.artifact = Some(artifact);
                progress(ProjectEditorCompositionPreparationPhase::Ready);
                return Ok(());
            }
            Err(error) => {
                report.diagnostics.push(ProjectEditorCompositionDiagnostic {
                    code: "project_editor_composition.cache_invalidated".to_string(),
                    stage: "cache_lookup".to_string(),
                    message: error.message,
                    path: Some(artifact_root.display().to_string()),
                    expected_identity: Some(identity_digest.clone()),
                    actual_identity: None,
                    next_action: "Rebuild the sealed composition artifact.".to_string(),
                });
                remove_owned_cache_entry(&cache_root, &artifact_root)?;
            }
        }
    }
    let target_root =
        prepare_composition_compilation_target_root(&cache_root, request, &lineage, report)?;
    let environment = vec![(
        OsString::from("CARGO_TARGET_DIR"),
        target_root.as_os_str().to_os_string(),
    )];
    let resolved_jobs = report
        .qos_decision
        .as_ref()
        .map(|decision| decision.resolved_jobs)
        .unwrap_or(request.qos_policy.min_jobs);
    run_required_step(
        report,
        "build_composition_release",
        &cargo,
        vec![
            "build".into(),
            "--release".into(),
            "--locked".into(),
            "--offline".into(),
            "--jobs".into(),
            resolved_jobs.to_string().into(),
        ],
        &generated_root,
        &environment,
        request,
        control,
        request.deadline_policy.release_build_hard_deadline_ms,
        Some(request.deadline_policy.release_build_soft_budget_ms),
    )?;

    let staging_executable = generated_executable(&target_root, &request.expected_identity)?;
    if !staging_executable.is_file() {
        return Err(CompositionBuildError::new(
            "project_editor_composition.executable_missing",
            "seal_artifact",
            "Generated composition build did not produce the expected executable.",
            Some(&staging_executable),
            "Inspect the bounded Cargo build report.",
        ));
    }
    progress(ProjectEditorCompositionPreparationPhase::Sealing);
    let actual_identity = query_descriptor(&staging_executable, report, request, control)?;
    validate_built_descriptor(&request.expected_identity, &actual_identity)?;
    let executable_hash = sha256_prefixed(&fs::read(&staging_executable).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.executable_read_failed",
            "seal_artifact",
            error.to_string(),
            Some(&staging_executable),
            "Rebuild the composition artifact.",
        )
    })?);
    let final_executable = artifact_root.join("bin").join(
        staging_executable
            .file_name()
            .unwrap_or_else(|| OsStr::new("editor.exe")),
    );
    let final_descriptor = artifact_root.join("composition-descriptor.json");
    let final_report = artifact_root.join("build-report.json");
    let descriptor = ProjectEditorCompositionDescriptor {
        schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
        identity: request.expected_identity.clone(),
        identity_digest: identity_digest.clone(),
        resolved_identity: resolved_identity.clone(),
        executable_hash: executable_hash.clone(),
        created_at: now_epoch_seconds(),
    };
    let artifact = ProjectEditorCompositionArtifact {
        schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
        executable_path: final_executable,
        descriptor_path: final_descriptor,
        build_report_path: final_report,
        descriptor: descriptor.clone(),
    };

    let staging_publish = staging_root.join("PublishedArtifact");
    fs::create_dir_all(staging_publish.join("bin")).map_err(|error| {
        io_error(
            "project_editor_composition.publish_prepare_failed",
            "publish",
            error,
            &staging_publish,
        )
    })?;
    fs::copy(
        &staging_executable,
        staging_publish
            .join("bin")
            .join(staging_executable.file_name().unwrap()),
    )
    .map_err(|error| {
        io_error(
            "project_editor_composition.publish_prepare_failed",
            "publish",
            error,
            &staging_publish,
        )
    })?;
    atomic_write_json(
        &staging_publish.join("composition-descriptor.json"),
        &descriptor,
    )?;
    report.status = ProjectEditorCompositionBuildStatus::Success;
    report.cache_status = ProjectEditorCompositionCacheStatus::Rebuilt;
    report.cleanup_status = "staging_published".to_string();
    report.artifact = Some(artifact.clone());
    report.artifact_size_bytes = Some(directory_size(&staging_publish)?);
    atomic_write_json(&staging_publish.join("build-report.json"), report)?;
    touch_cache_entry(
        &staging_publish,
        &identity_digest,
        &request.expected_identity.project_id,
        0,
    )?;
    let mut size = directory_size(&staging_publish)?;
    report.artifact_size_bytes = Some(size);
    progress(ProjectEditorCompositionPreparationPhase::Ready);
    atomic_write_json(&staging_publish.join("build-report.json"), report)?;
    touch_cache_entry(
        &staging_publish,
        &identity_digest,
        &request.expected_identity.project_id,
        size,
    )?;
    let measured = directory_size(&staging_publish)?;
    if measured != size {
        size = measured;
        report.artifact_size_bytes = Some(size);
        atomic_write_json(&staging_publish.join("build-report.json"), report)?;
        touch_cache_entry(
            &staging_publish,
            &identity_digest,
            &request.expected_identity.project_id,
            size,
        )?;
    }
    ensure_cache_capacity(
        &cache_root,
        &request.expected_identity.project_id,
        key,
        size,
        &request.cache_policy,
    )?;
    publish_directory(&staging_publish, &artifact_root)?;
    let _ = cleanup_owned_directory(&cache_root, &staging_root);
    *staging_slot = None;
    report.artifact_size_bytes = Some(size);
    report.artifact = Some(artifact);
    Ok(())
}

fn composition_compilation_compatibility_digest(
    request: &ProjectEditorCompositionBuildRequest,
    lineage: &GeneratedCompositionLockLineage,
) -> String {
    sha256_prefixed(
        format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            COMPILATION_CACHE_SCHEMA_VERSION,
            request.expected_identity.toolchain_identity,
            request.expected_identity.target_triple,
            request.expected_identity.profile,
            request.expected_identity.normalized_manifest_digest,
            request.expected_identity.normalized_dependency_digest,
            lineage.resolved_graph_digest,
            request.expected_identity.engine_sdk_digest,
        )
        .as_bytes(),
    )
}

fn prepare_composition_compilation_target_root(
    cache_root: &Path,
    request: &ProjectEditorCompositionBuildRequest,
    lineage: &GeneratedCompositionLockLineage,
    report: &mut ProjectEditorCompositionBuildReport,
) -> Result<PathBuf, CompositionBuildError> {
    let digest = composition_compilation_compatibility_digest(request, lineage);
    let canonical_cache_root = canonical_regular_directory(cache_root, "composition cache root")?;
    let canonical_target_anchor_digest = sha256_prefixed(
        canonical_cache_root
            .to_string_lossy()
            .replace('\\', "/")
            .as_bytes(),
    );
    let target_root = cache_root
        .join("ct")
        .join(&digest.trim_start_matches("sha256:")[..32])
        .join(&canonical_target_anchor_digest.trim_start_matches("sha256:")[..32]);
    let existed = target_root.exists();
    ensure_owned_directory(&target_root)?;
    let canonical_target_root =
        canonical_regular_directory(&target_root, "compilation target root")?;
    let canonical_target_root_digest = sha256_prefixed(
        canonical_target_root
            .to_string_lossy()
            .replace('\\', "/")
            .as_bytes(),
    );
    let marker_path = target_root.join("affinity.json");
    let expected = CompilationCacheAffinityMarker {
        schema_version: COMPILATION_CACHE_SCHEMA_VERSION.to_string(),
        compatibility_digest: digest.clone(),
        canonical_target_anchor_digest: canonical_target_anchor_digest.clone(),
        canonical_target_root_digest: canonical_target_root_digest.clone(),
        creator_identity: request.expected_identity.toolchain_identity.clone(),
    };
    let affinity = if marker_path.is_file() {
        let actual: CompilationCacheAffinityMarker = read_json(&marker_path)?;
        if actual != expected {
            return Err(CompositionBuildError::new(
                "project_editor_composition.compilation_cache_affinity_invalid",
                "prepare_compilation_cache",
                "Compilation cache affinity marker does not match its canonical target root.",
                Some(&marker_path),
                "Use a new application-owned target root; do not consume copied Cargo intermediates.",
            ));
        }
        ProjectEditorCompositionCompilationCacheAffinity::SameRootHit
    } else {
        atomic_write_json(&marker_path, &expected)?;
        if existed
            || compilation_has_other_affinity(cache_root, &digest, &canonical_target_anchor_digest)?
        {
            ProjectEditorCompositionCompilationCacheAffinity::PathAffineMiss
        } else {
            ProjectEditorCompositionCompilationCacheAffinity::Cold
        }
    };
    report.compilation_cache_compatibility_digest = Some(digest);
    report.compilation_cache_affinity = affinity;
    report.canonical_target_anchor_digest = Some(canonical_target_anchor_digest);
    report.canonical_target_root_digest = Some(canonical_target_root_digest);
    report.cross_root_portable = false;
    Ok(target_root)
}

fn compilation_has_other_affinity(
    cache_root: &Path,
    compatibility_digest: &str,
    current_anchor_digest: &str,
) -> Result<bool, CompositionBuildError> {
    let compatibility_root = cache_root
        .join("ct")
        .join(&compatibility_digest.trim_start_matches("sha256:")[..32]);
    if !compatibility_root.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(&compatibility_root).map_err(|error| {
        io_error(
            "project_editor_composition.compilation_identity_read_failed",
            "prepare_compilation_cache",
            error,
            &compatibility_root,
        )
    })? {
        let entry = entry.map_err(|error| {
            io_error(
                "project_editor_composition.compilation_identity_read_failed",
                "prepare_compilation_cache",
                error,
                &compatibility_root,
            )
        })?;
        if entry.path().join("affinity.json").is_file()
            && entry.file_name().to_string_lossy()
                != &current_anchor_digest.trim_start_matches("sha256:")[..32]
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_staging_identity(
    expected: &ProjectEditorCompositionIdentity,
    plan: &crate::project_runtime_player_staging::ProjectRuntimePlayerStagingPlan,
) -> Result<(), CompositionBuildError> {
    let actual = [
        (
            "projectId",
            expected.project_id.as_str(),
            plan.manifest.project_id.as_str(),
        ),
        (
            "moduleId",
            expected.module_id.as_str(),
            plan.manifest.runtime_module.module_id.as_str(),
        ),
        (
            "interfaceVersion",
            expected.interface_version.as_str(),
            plan.manifest.runtime_module.interface_version.as_str(),
        ),
        (
            "normalizedManifestDigest",
            expected.normalized_manifest_digest.as_str(),
            plan.normalized_manifest_digest.as_str(),
        ),
        (
            "normalizedDependencyDigest",
            expected.normalized_dependency_digest.as_str(),
            plan.normalized_dependency_digest.as_str(),
        ),
        (
            "dependencyLockDigest",
            expected.dependency_lock_digest.as_str(),
            plan.trusted_lock_digest.as_str(),
        ),
    ];
    if let Some((field, expected, actual)) = actual
        .iter()
        .find(|(_, expected, actual)| expected != actual)
    {
        return Err(CompositionBuildError::new(
            "project_editor_composition.staging_identity_mismatch",
            "production_staging_plan",
            format!("Composition {field} expected '{expected}', actual '{actual}'."),
            None,
            "Regenerate trust and composition identity from the current staging plan.",
        ));
    }
    Ok(())
}

fn generated_lock_input(
    request: &ProjectEditorCompositionBuildRequest,
    plan: &crate::project_runtime_player_staging::ProjectRuntimePlayerStagingPlan,
    generated_root: &Path,
) -> Result<GeneratedCompositionLockInput, CompositionBuildError> {
    let generated_manifest = fs::read(generated_root.join("Cargo.toml")).map_err(|error| {
        io_error(
            "project_editor_composition.lock_lineage_input_mismatch",
            "prepare_lock_lineage",
            error,
            generated_root,
        )
    })?;
    let generated_manifest_template_digest =
        canonical_generated_manifest_template_digest(&generated_manifest)?;
    Ok(GeneratedCompositionLockInput {
        schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
        cargo_identity: request.cargo_identity.clone(),
        toolchain_identity: request.expected_identity.toolchain_identity.clone(),
        target_triple: request.expected_identity.target_triple.clone(),
        profile: request.expected_identity.profile.clone(),
        generated_feature_set: vec!["real-window".to_string(), "real-wgpu-surface".to_string()],
        generated_manifest_template_digest,
        runtime_module_manifest_digest: plan.normalized_manifest_digest.clone(),
        normalized_dependency_identity_digest: plan.normalized_dependency_digest.clone(),
        engine_sdk_lock_digest: plan.trusted_lock_digest.clone(),
        trusted_engine_manifest_set_digest: request.expected_identity.engine_sdk_digest.clone(),
    })
}

fn canonical_generated_manifest_template_digest(
    manifest_bytes: &[u8],
) -> Result<String, CompositionBuildError> {
    let manifest_text = std::str::from_utf8(manifest_bytes).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.lock_lineage_input_mismatch",
            "prepare_lock_lineage",
            error.to_string(),
            None,
            "Regenerate the generated composition manifest as UTF-8 TOML.",
        )
    })?;
    let mut manifest: toml::Value = toml::from_str(manifest_text).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.lock_lineage_input_mismatch",
            "prepare_lock_lineage",
            error.to_string(),
            None,
            "Regenerate the generated composition manifest from the trusted template.",
        )
    })?;
    if let Some(dependencies) = manifest
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
    {
        for (name, dependency) in dependencies {
            if let Some(table) = dependency.as_table_mut() {
                if table.contains_key("path") {
                    table.insert(
                        "path".to_string(),
                        toml::Value::String(format!("<generated-dependency:{name}>")),
                    );
                }
            }
        }
    }
    toml::to_string(&manifest)
        .map(|canonical| sha256_prefixed(canonical.as_bytes()))
        .map_err(|error| {
            CompositionBuildError::new(
                "project_editor_composition.lock_lineage_input_mismatch",
                "prepare_lock_lineage",
                error.to_string(),
                None,
                "Repair the generated composition manifest template.",
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn prepare_generated_lock_lineage(
    cache_root: &Path,
    generated_root: &Path,
    cargo: &Path,
    request: &ProjectEditorCompositionBuildRequest,
    control: &ProjectEditorCompositionPreparationControl,
    report: &mut ProjectEditorCompositionBuildReport,
    lock_input_digest: String,
) -> Result<GeneratedCompositionLockLineage, CompositionBuildError> {
    let lineage_root = cache_root
        .join("locks")
        .join(lock_input_digest.trim_start_matches("sha256:"));
    let stored_lock = lineage_root.join("Cargo.lock");
    let stored_lineage = lineage_root.join("lineage.json");
    if stored_lock.is_file() && stored_lineage.is_file() {
        let lock_bytes = fs::read(&stored_lock).map_err(|error| {
            io_error(
                "project_editor_composition.lock_lineage_raw_digest_mismatch",
                "prepare_lock_lineage",
                error,
                &stored_lock,
            )
        })?;
        let stored: GeneratedCompositionLockLineage = read_json(&stored_lineage)?;
        stored.validate().map_err(contract_build_error)?;
        let root_name = generated_package_name(&request.expected_identity)?;
        let actual =
            generated_composition_lock_lineage(&lock_bytes, &root_name, lock_input_digest.clone())
                .map_err(contract_build_error)?;
        if stored != actual {
            return Err(CompositionBuildError::new(
                if stored.raw_lock_digest != actual.raw_lock_digest {
                    "project_editor_composition.lock_lineage_raw_digest_mismatch"
                } else {
                    "project_editor_composition.lock_lineage_graph_digest_mismatch"
                },
                "prepare_lock_lineage",
                "Stored generated lock lineage does not match its sealed Cargo.lock.",
                Some(&lineage_root),
                "Remove the invalid application-owned lineage entry and regenerate it.",
            ));
        }
        atomic_write(&generated_root.join("Cargo.lock"), &lock_bytes)?;
        return Ok(stored);
    }
    if lineage_root.exists() {
        return Err(CompositionBuildError::new(
            "project_editor_composition.lock_lineage_collision",
            "prepare_lock_lineage",
            "Generated lock lineage entry is incomplete.",
            Some(&lineage_root),
            "Remove the invalid application-owned lineage entry and retry.",
        ));
    }
    run_required_step(
        report,
        "generate_composition_lock",
        cargo,
        vec!["generate-lockfile".into(), "--offline".into()],
        generated_root,
        &[],
        request,
        control,
        request.deadline_policy.generate_lock_hard_deadline_ms,
        None,
    )?;
    let lock_path = generated_root.join("Cargo.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|error| {
        io_error(
            "project_editor_composition.lock_lineage_raw_digest_mismatch",
            "prepare_lock_lineage",
            error,
            &lock_path,
        )
    })?;
    let root_name = generated_package_name(&request.expected_identity)?;
    let lineage = generated_composition_lock_lineage(&lock_bytes, &root_name, lock_input_digest)
        .map_err(contract_build_error)?;
    let staging = cache_root.join("locks").join(format!(
        ".staging-{}-{}",
        std::process::id(),
        BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    ensure_owned_directory(&staging)?;
    atomic_write(&staging.join("Cargo.lock"), &lock_bytes)?;
    atomic_write_json(&staging.join("lineage.json"), &lineage)?;
    if lineage_root.exists() {
        let _ = cleanup_owned_directory(cache_root, &staging);
        return Err(CompositionBuildError::new(
            "project_editor_composition.lock_lineage_collision",
            "prepare_lock_lineage",
            "Generated lock lineage appeared concurrently.",
            Some(&lineage_root),
            "Validate the existing lineage before retrying.",
        ));
    }
    publish_directory(&staging, &lineage_root).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.lock_lineage_publish_failed",
            "prepare_lock_lineage",
            error.message,
            Some(&lineage_root),
            "Retry with a writable application-owned build root.",
        )
    })?;
    Ok(lineage)
}

fn contract_build_error(
    error: crate::ProjectEditorCompositionContractError,
) -> CompositionBuildError {
    CompositionBuildError::new(
        &error.code,
        "composition_contract",
        error.message,
        None,
        "Regenerate the composition input from trusted source identities.",
    )
}

fn write_generated_composition(
    staging_root: &Path,
    sdk_root: &Path,
    project_package: &str,
    identity: &ProjectEditorCompositionIdentity,
) -> Result<(), CompositionBuildError> {
    let generated = staging_root.join("GeneratedEditor");
    fs::create_dir_all(generated.join("src")).map_err(|error| {
        io_error(
            "project_editor_composition.generated_source_write_failed",
            "generate_source",
            error,
            &generated,
        )
    })?;
    let editor_root = trusted_sdk_crate(sdk_root, "editor_window_winit")?;
    let engine_root = trusted_sdk_crate(sdk_root, "engine_runtime")?;
    let mut manifest = toml::map::Map::new();
    let package_name = generated_package_name(identity)?;
    let mut package = toml::toml! {
        version = "0.0.2"
        edition = "2021"
        publish = false
    };
    package.insert("name".to_string(), toml::Value::String(package_name));
    manifest.insert("package".to_string(), toml::Value::Table(package));
    let mut dependencies = toml::map::Map::new();
    dependencies.insert(
        "editor_window_winit".to_string(),
        path_dependency(&editor_root, None, &["real-window", "real-wgpu-surface"]),
    );
    dependencies.insert(
        "engine_runtime".to_string(),
        path_dependency(&engine_root, None, &[]),
    );
    dependencies.insert(
        "project_runtime".to_string(),
        path_dependency(
            &staging_root.join("RuntimeModuleBuild"),
            Some(project_package),
            &[],
        ),
    );
    manifest.insert("dependencies".to_string(), toml::Value::Table(dependencies));
    let manifest_text = toml::to_string(&toml::Value::Table(manifest)).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.generated_manifest_invalid",
            "generate_source",
            error.to_string(),
            Some(&generated),
            "Repair the generated composition template.",
        )
    })?;
    atomic_write(&generated.join("Cargo.toml"), manifest_text.as_bytes())?;
    atomic_write(
        &generated.join("src/main.rs"),
        generated_main(identity).as_bytes(),
    )
}

fn generated_main(identity: &ProjectEditorCompositionIdentity) -> String {
    let identity_literal = format!(
        r#"editor_window_winit::ProjectEditorCompositionIdentity {{
        schema_version: {:?}.to_string(),
        project_id: {:?}.to_string(),
        module_id: descriptor.module_id.clone(),
        interface_version: descriptor.interface_version.clone(),
        aot_content_digest: descriptor.aot_content_digest.clone(),
        editor_build_identity: {:?}.to_string(),
        engine_sdk_digest: {:?}.to_string(),
        toolchain_identity: {:?}.to_string(),
        target_triple: {:?}.to_string(),
        profile: {:?}.to_string(),
        normalized_manifest_digest: {:?}.to_string(),
        normalized_dependency_digest: {:?}.to_string(),
        dependency_lock_digest: {:?}.to_string(),
    }}"#,
        identity.schema_version,
        identity.project_id,
        identity.editor_build_identity,
        identity.engine_sdk_digest,
        identity.toolchain_identity,
        identity.target_triple,
        identity.profile,
        identity.normalized_manifest_digest,
        identity.normalized_dependency_digest,
        identity.dependency_lock_digest,
    );
    r#"use std::sync::Arc;

fn linked() -> Arc<engine_runtime::project_runtime_module::LinkedProjectRuntimeSet> {
    Arc::new(project_runtime::linked_set().expect("project RuntimeModule linked_set must be valid"))
}

fn identity() -> editor_window_winit::ProjectEditorCompositionIdentity {
    let linked = linked();
    let descriptor = linked.only_descriptor().expect("generated composition must be singleton");
    __PROJECT_EDITOR_COMPOSITION_IDENTITY__
}

fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--describe-project-runtime") {
        let linked = linked();
        let descriptor = linked.only_descriptor().expect("generated composition must be singleton");
        println!("{}", descriptor.module_id);
        println!("{}", descriptor.interface_version);
        println!("{}", descriptor.aot_content_digest);
        return;
    }
    if let Some(index) = args.iter().position(|arg| arg == "--qualify-project-runtime") {
        let project_root = args.get(index + 1)
            .map(std::path::PathBuf::from)
            .expect("--qualify-project-runtime requires a project root");
        let report = editor_window_winit::qualify_and_seal_project_editor_composition_headless(
            &project_root, linked(), identity(),
        );
        println!("{}", editor_window_winit::project_editor_composition_qualification_report_json(&report)
            .expect("qualification report must serialize"));
        std::process::exit(if report.status == "passed" { 0 } else { 1 });
    }
    if let Some(index) = args.iter().position(|arg| arg == "--qualify-project-runtime-real-window") {
        let request_path = args.get(index + 1)
            .map(std::path::PathBuf::from)
            .expect("--qualify-project-runtime-real-window requires a request path");
        let report = editor_window_winit::qualify_and_seal_project_editor_composition_real_window(
            &request_path, linked(), identity(),
        );
        println!("{}", editor_window_winit::project_editor_composition_real_window_report_json(&report)
            .expect("real-window qualification report must serialize"));
        std::process::exit(if report.status == "passed" { 0 } else { 1 });
    }
    if let Some(index) = args.iter().position(|arg| arg == "--run-production-authority-scenario") {
        let scenario_path = args.get(index + 1)
            .map(std::path::PathBuf::from)
            .expect("--run-production-authority-scenario requires a scenario path");
        let scenario = editor_window_winit::ProductionAuthorityScenario::load(&scenario_path)
            .expect("production authority scenario must be valid");
        let outcome = editor_window_winit::run_real_project_editor_composition_authority(
            editor_window_winit::RealProjectEditorCompositionAuthorityOptions {
                authority: editor_window_winit::RealNativeEditorAuthorityOptions {
                    physical_width: scenario.physical_width,
                    physical_height: scenario.physical_height,
                    report_level: editor_window_winit::EditorReachabilityReportLevel::Trace,
                    project_root: Some(scenario.project_root.clone()),
                    workspace_layout_store_root: Some(
                        scenario.workspace_layout_store_root.clone(),
                    ),
                    click_widget_id: None,
                    wheel_delta: None,
                    drag_target_widget_id: None,
                    drag_delta: None,
                    scenario_path: Some(scenario_path),
                },
                linked_project_runtimes: linked(),
                identity: identity(),
            },
        );
        let report = editor_window_winit::production_authority_report_or_fail_closed(
            &scenario,
            outcome.production_authority_report,
        );
        println!("{}", editor_window_winit::production_authority_report_json(&report)
            .expect("production authority report must serialize"));
        std::process::exit(if report.status == "passed" { 0 } else { 1 });
    }
    let ticket = editor_window_winit::project_editor_handoff_ticket_from_args(args)
        .expect("project Editor handoff launch input must be valid");
    let _ = match ticket {
        Some(ticket_path) =>
            editor_window_winit::run_real_native_editor_window_with_project_composition_and_handoff(
                linked(), identity(), ticket_path,
            ),
        None => editor_window_winit::run_real_native_editor_window_with_project_composition(
            linked(), identity(),
        ),
    };
}
"#
    .replace("__PROJECT_EDITOR_COMPOSITION_IDENTITY__", &identity_literal)
}

fn path_dependency(path: &Path, package: Option<&str>, features: &[&str]) -> toml::Value {
    let mut table = toml::map::Map::new();
    table.insert(
        "path".to_string(),
        toml::Value::String(path.display().to_string()),
    );
    if let Some(package) = package {
        table.insert(
            "package".to_string(),
            toml::Value::String(package.to_string()),
        );
    }
    if !features.is_empty() {
        table.insert(
            "features".to_string(),
            toml::Value::Array(
                features
                    .iter()
                    .map(|value| toml::Value::String((*value).to_string()))
                    .collect(),
            ),
        );
    }
    toml::Value::Table(table)
}

fn trusted_sdk_crate(sdk_root: &Path, name: &str) -> Result<PathBuf, CompositionBuildError> {
    let trusted_root = canonical_regular_directory(sdk_root, "Engine SDK root")?;
    let path = trusted_root.join("crates").join(name);
    let canonical = canonical_regular_directory(&path, &format!("Engine SDK crate {name}"))?;
    if canonical.parent().and_then(Path::parent) != Some(trusted_root.as_path()) {
        return Err(CompositionBuildError::new(
            "project_editor_composition.engine_sdk_dependency_untrusted",
            "generate_source",
            format!("Engine SDK crate '{name}' escaped the trusted SDK root."),
            Some(&canonical),
            "Repair the trusted Engine SDK.",
        ));
    }
    Ok(canonical)
}

fn run_required_step(
    report: &mut ProjectEditorCompositionBuildReport,
    stage: &str,
    executable: &Path,
    args: Vec<OsString>,
    current_dir: &Path,
    environment: &[(OsString, OsString)],
    request: &ProjectEditorCompositionBuildRequest,
    control: &ProjectEditorCompositionPreparationControl,
    hard_deadline_ms: u64,
    soft_budget_ms: Option<u64>,
) -> Result<(), CompositionBuildError> {
    let process = run_bounded_child_process_cancellable(
        BoundedChildProcessRequest {
            executable: executable.to_path_buf(),
            args: args.clone(),
            current_dir: current_dir.to_path_buf(),
            environment: environment.to_vec(),
            timeout: Duration::from_millis(hard_deadline_ms),
            stdout_capture_limit_bytes: request.capture_limit_bytes.min(1024 * 1024),
            stderr_capture_limit_bytes: request.capture_limit_bytes.min(1024 * 1024),
            priority: BoundedChildProcessPriority::BelowNormal,
        },
        control.process_cancellation(),
    );
    let passed = process.exit_reason == BoundedChildProcessExitReason::Completed
        && process.exit_code == Some(0)
        && process.reader_join_error.is_none();
    let stderr = process.stderr_summary.clone();
    if soft_budget_ms.is_some_and(|budget| process.elapsed_ms > u128::from(budget)) {
        report.release_build_soft_budget_exceeded = true;
        report.release_build_soft_budget_exceeded_at_ms = Some(process.elapsed_ms);
        report.diagnostics.push(ProjectEditorCompositionDiagnostic {
            code: "project_editor_composition.release_build_soft_budget_exceeded".to_string(),
            stage: stage.to_string(),
            message: "Composition release build exceeded its soft budget and continued."
                .to_string(),
            path: Some(current_dir.display().to_string()),
            expected_identity: report.identity_digest.clone(),
            actual_identity: None,
            next_action: "Continue waiting or cancel; this warning is not a build failure."
                .to_string(),
        });
    }
    record_process_evidence(report, stage, &process);
    report.steps.push(ProjectEditorCompositionBuildStep {
        stage: stage.to_string(),
        command: std::iter::once(executable.display().to_string())
            .chain(
                args.iter()
                    .map(|value| value.to_string_lossy().into_owned()),
            )
            .collect(),
        timeout_ms: hard_deadline_ms,
        process,
    });
    if !passed {
        let reason = report.steps.last().unwrap().process.exit_reason;
        return Err(CompositionBuildError::new(
            match reason {
                BoundedChildProcessExitReason::Cancelled => "project_editor_composition.cancelled",
                BoundedChildProcessExitReason::Timeout if stage == "build_composition_release" => {
                    "project_editor_composition.release_build_hard_timeout"
                }
                BoundedChildProcessExitReason::Timeout => {
                    "project_editor_composition.build_step_hard_timeout"
                }
                BoundedChildProcessExitReason::SpawnFailed => {
                    "project_editor_composition.build_process_spawn_failed"
                }
                BoundedChildProcessExitReason::WaitFailed => {
                    "project_editor_composition.build_process_wait_failed"
                }
                BoundedChildProcessExitReason::Completed
                | BoundedChildProcessExitReason::Failed => {
                    "project_editor_composition.build_step_failed"
                }
            },
            stage,
            format!("Generated composition build step failed: {stderr}"),
            Some(current_dir),
            "Inspect the bounded process result and fix the generated build inputs.",
        ));
    }
    Ok(())
}

fn query_descriptor(
    executable: &Path,
    report: &mut ProjectEditorCompositionBuildReport,
    request: &ProjectEditorCompositionBuildRequest,
    control: &ProjectEditorCompositionPreparationControl,
) -> Result<ProjectEditorCompositionIdentity, CompositionBuildError> {
    let process = run_bounded_child_process_cancellable(
        BoundedChildProcessRequest {
            executable: executable.to_path_buf(),
            args: vec![OsString::from("--describe-project-runtime")],
            current_dir: executable
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            environment: Vec::new(),
            timeout: Duration::from_millis(
                request.deadline_policy.descriptor_query_hard_deadline_ms,
            ),
            stdout_capture_limit_bytes: request.capture_limit_bytes.min(64 * 1024),
            stderr_capture_limit_bytes: request.capture_limit_bytes.min(64 * 1024),
            priority: BoundedChildProcessPriority::BelowNormal,
        },
        control.process_cancellation(),
    );
    let passed = process.exit_reason == BoundedChildProcessExitReason::Completed
        && process.exit_code == Some(0)
        && process.reader_join_error.is_none();
    let stdout = process.stdout_summary.clone();
    let stderr = process.stderr_summary.clone();
    record_process_evidence(report, "query_composition_descriptor", &process);
    report.steps.push(ProjectEditorCompositionBuildStep {
        stage: "query_composition_descriptor".to_string(),
        command: vec![
            executable.display().to_string(),
            "--describe-project-runtime".to_string(),
        ],
        timeout_ms: request.deadline_policy.descriptor_query_hard_deadline_ms,
        process,
    });
    if !passed {
        let reason = report.steps.last().unwrap().process.exit_reason;
        return Err(CompositionBuildError::new(
            match reason {
                BoundedChildProcessExitReason::Cancelled => "project_editor_composition.cancelled",
                BoundedChildProcessExitReason::Timeout => {
                    "project_editor_composition.descriptor_query_hard_timeout"
                }
                BoundedChildProcessExitReason::SpawnFailed => {
                    "project_editor_composition.build_process_spawn_failed"
                }
                BoundedChildProcessExitReason::WaitFailed => {
                    "project_editor_composition.build_process_wait_failed"
                }
                BoundedChildProcessExitReason::Completed
                | BoundedChildProcessExitReason::Failed => {
                    "project_editor_composition.descriptor_query_failed"
                }
            },
            "query_descriptor",
            format!("Generated composition descriptor query failed: {stderr}"),
            Some(executable),
            "Rebuild the generated composition and inspect its bounded output.",
        ));
    }
    let lines = stdout.lines().collect::<Vec<_>>();
    if lines.len() != 3 {
        return Err(CompositionBuildError::new(
            "project_editor_composition.descriptor_query_invalid",
            "query_descriptor",
            "Generated composition descriptor query did not return exactly three fields.",
            Some(executable),
            "Repair the generated descriptor query contract.",
        ));
    }
    let mut identity = request.expected_identity.clone();
    identity.module_id = lines[0].to_string();
    identity.interface_version = lines[1].to_string();
    identity.aot_content_digest = lines[2].to_string();
    Ok(identity)
}

fn record_process_evidence(
    report: &mut ProjectEditorCompositionBuildReport,
    stage: &str,
    process: &BoundedChildProcessResult,
) {
    report
        .stage_durations_ms
        .insert(stage.to_string(), process.elapsed_ms);
    report.effective_priority = process.priority.effective.map(|priority| match priority {
        BoundedChildProcessPriority::Normal => ProjectEditorCompositionProcessPriority::Normal,
        BoundedChildProcessPriority::BelowNormal => {
            ProjectEditorCompositionProcessPriority::BelowNormal
        }
    });
    report.priority_applied = process.priority.applied;
    report.cancellation_requested |=
        process.exit_reason == BoundedChildProcessExitReason::Cancelled;
    report.process_tree_terminated |= process.ownership.termination_requested;
    report.output_readers_joined |= process.ownership.output_readers_joined;
    report.root_wait_completed |= process.ownership.root_process_wait_completed;
    report.process_group_released |= process.ownership.process_group_release_completed;
    report.owned_process_cleanup_confirmed |= process.owned_process_cleanup_confirmed();
}

fn validate_built_descriptor(
    expected: &ProjectEditorCompositionIdentity,
    actual: &ProjectEditorCompositionIdentity,
) -> Result<(), CompositionBuildError> {
    if expected.module_id != actual.module_id
        || expected.interface_version != actual.interface_version
        || expected.aot_content_digest != actual.aot_content_digest
    {
        return Err(CompositionBuildError::new(
            "project_editor_composition.module_identity_mismatch",
            "seal_artifact",
            format!(
                "Generated module identity mismatch: expected {}/{}/{}, actual {}/{}/{}.",
                expected.module_id,
                expected.interface_version,
                expected.aot_content_digest,
                actual.module_id,
                actual.interface_version,
                actual.aot_content_digest
            ),
            None,
            "Rebuild from the exact trusted ProjectRust identity.",
        ));
    }
    Ok(())
}

pub(crate) fn load_cached_artifact(
    artifact_root: &Path,
    expected: &ProjectEditorCompositionIdentity,
    identity_digest: &str,
    expected_resolved_identity: &ProjectEditorCompositionResolvedIdentity,
) -> Result<(ProjectEditorCompositionArtifact, u64), CompositionBuildError> {
    let descriptor_path = artifact_root.join("composition-descriptor.json");
    let report_path = artifact_root.join("build-report.json");
    let descriptor: ProjectEditorCompositionDescriptor = read_json(&descriptor_path)?;
    if descriptor.schema_version != PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION
        || descriptor.identity != *expected
        || descriptor.identity_digest != identity_digest
        || descriptor.resolved_identity != *expected_resolved_identity
    {
        return Err(cache_invalid(
            "Cached composition descriptor identity is stale.",
            &descriptor_path,
        ));
    }
    let executable = artifact_root
        .join("bin")
        .join(generated_artifact_executable_name(expected)?);
    let actual_hash = sha256_prefixed(&fs::read(&executable).map_err(|error| {
        io_error(
            "project_editor_composition.cache_invalid",
            "cache_lookup",
            error,
            &executable,
        )
    })?);
    if actual_hash != descriptor.executable_hash || !report_path.is_file() {
        return Err(cache_invalid(
            "Cached composition executable seal is invalid.",
            &executable,
        ));
    }
    let size = directory_size(artifact_root)?;
    Ok((
        ProjectEditorCompositionArtifact {
            schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
            executable_path: executable,
            descriptor_path,
            build_report_path: report_path,
            descriptor,
        },
        size,
    ))
}

fn ensure_cache_capacity(
    cache_root: &Path,
    project_id: &str,
    incoming_key: &str,
    incoming_size: u64,
    policy: &crate::ProjectEditorCompositionCachePolicy,
) -> Result<(), CompositionBuildError> {
    let cache = cache_root.join("cache");
    let mut entries = Vec::new();
    for entry in fs::read_dir(&cache).map_err(|error| {
        io_error(
            "project_editor_composition.cache_read_failed",
            "cache_prune",
            error,
            &cache,
        )
    })? {
        let entry = entry.map_err(|error| {
            io_error(
                "project_editor_composition.cache_read_failed",
                "cache_prune",
                error,
                &cache,
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            io_error(
                "project_editor_composition.cache_read_failed",
                "cache_prune",
                error,
                &path,
            )
        })?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(cache_invalid(
                "Composition cache contains an unsafe entry.",
                &path,
            ));
        }
        if entry.file_name() == incoming_key {
            continue;
        }
        let cache_entry: CacheEntryMetadata = read_json(&path.join("cache-entry.json"))?;
        let pinned = cache_root
            .join("pins")
            .join(format!("{}.pin", entry.file_name().to_string_lossy()))
            .is_file();
        entries.push((path, cache_entry, pinned));
    }
    entries.sort_by_key(|(_, metadata, _)| metadata.last_used_at);
    let mut global = entries
        .iter()
        .map(|(_, entry, _)| entry.size_bytes)
        .sum::<u64>();
    let mut project = entries
        .iter()
        .filter(|(_, entry, _)| entry.project_id == project_id)
        .map(|(_, entry, _)| entry.size_bytes)
        .sum::<u64>();
    let needs_prune = global.saturating_add(incoming_size) > policy.global_soft_limit_bytes
        || project.saturating_add(incoming_size) > policy.per_project_soft_limit_bytes;
    if needs_prune {
        for (path, entry, pinned) in &entries {
            if *pinned {
                continue;
            }
            if global.saturating_add(incoming_size) <= policy.global_soft_limit_bytes
                && project.saturating_add(incoming_size) <= policy.per_project_soft_limit_bytes
            {
                break;
            }
            remove_owned_cache_entry(cache_root, path)?;
            global = global.saturating_sub(entry.size_bytes);
            if entry.project_id == project_id {
                project = project.saturating_sub(entry.size_bytes);
            }
        }
    }
    if global.saturating_add(incoming_size) > policy.global_hard_limit_bytes
        || project.saturating_add(incoming_size) > policy.per_project_hard_limit_bytes
    {
        return Err(CompositionBuildError::new(
            "project_editor_composition.cache_capacity_exceeded",
            "cache_prune",
            "Composition cache hard limit cannot be satisfied without removing pinned artifacts.",
            Some(cache_root),
            "Close active compositions or increase the controlled cache quota.",
        ));
    }
    Ok(())
}

pub(crate) fn touch_cache_entry(
    artifact_root: &Path,
    identity_digest: &str,
    project_id: &str,
    size_bytes: u64,
) -> Result<(), CompositionBuildError> {
    atomic_write_json(
        &artifact_root.join("cache-entry.json"),
        &CacheEntryMetadata {
            schema_version: "project-editor-composition-cache-entry.v1".to_string(),
            identity_digest: identity_digest.to_string(),
            project_id: project_id.to_string(),
            size_bytes,
            last_used_at: now_epoch_seconds(),
        },
    )
}

pub(crate) fn publish_directory(
    staging: &Path,
    artifact: &Path,
) -> Result<(), CompositionBuildError> {
    if artifact.exists() {
        return Err(CompositionBuildError::new(
            "project_editor_composition.publish_collision",
            "publish",
            "Composition artifact appeared while publishing.",
            Some(artifact),
            "Retry preparation and reuse the validated cache entry.",
        ));
    }
    fs::rename(staging, artifact).map_err(|error| {
        io_error(
            "project_editor_composition.publish_failed",
            "publish",
            error,
            artifact,
        )
    })
}

pub(crate) fn remove_owned_cache_entry(
    cache_root: &Path,
    target: &Path,
) -> Result<(), CompositionBuildError> {
    let canonical_cache = cache_root.canonicalize().map_err(|error| {
        io_error(
            "project_editor_composition.cleanup_failed",
            "cleanup",
            error,
            cache_root,
        )
    })?;
    let parent = target
        .parent()
        .ok_or_else(|| cache_invalid("Cache entry has no parent.", target))?;
    let canonical_parent = parent.canonicalize().map_err(|error| {
        io_error(
            "project_editor_composition.cleanup_failed",
            "cleanup",
            error,
            parent,
        )
    })?;
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        io_error(
            "project_editor_composition.cleanup_failed",
            "cleanup",
            error,
            target,
        )
    })?;
    if canonical_parent != canonical_cache.join("cache")
        || is_link_or_reparse(&metadata)
        || !metadata.is_dir()
    {
        return Err(CompositionBuildError::new(
            "project_editor_composition.cleanup_scope_rejected",
            "cleanup",
            "Composition cleanup target is not a regular direct cache child.",
            Some(target),
            "Inspect the cache root without following the unsafe entry.",
        ));
    }
    fs::remove_dir_all(target).map_err(|error| {
        io_error(
            "project_editor_composition.cleanup_failed",
            "cleanup",
            error,
            target,
        )
    })
}

fn cleanup_owned_directory(owner_root: &Path, target: &Path) -> Result<String, String> {
    if !target.exists() {
        return Ok("not_required".to_string());
    }
    let owner = owner_root
        .canonicalize()
        .map_err(|_| "retained_by_host_policy".to_string())?;
    let parent = target
        .parent()
        .and_then(|value| value.canonicalize().ok())
        .ok_or_else(|| "retained_by_host_policy".to_string())?;
    let metadata =
        fs::symlink_metadata(target).map_err(|_| "retained_by_host_policy".to_string())?;
    if !parent.starts_with(&owner) || is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err("retained_by_host_policy".to_string());
    }
    fs::remove_dir_all(target)
        .map(|_| "removed".to_string())
        .map_err(|_| "retained_by_host_policy".to_string())
}

fn prepare_build_root(
    build_root: &Path,
    project_root: &Path,
) -> Result<PathBuf, CompositionBuildError> {
    let requested_build = normalize_windows_verbatim_path(build_root.to_path_buf());
    if requested_build.starts_with(project_root) || project_root.starts_with(&requested_build) {
        return Err(CompositionBuildError::new(
            "project_editor_composition.build_root_scope_rejected",
            "validate_request",
            "Composition build root and project root must be disjoint.",
            Some(build_root),
            "Choose an application-owned composition build root.",
        ));
    }
    fs::create_dir_all(build_root).map_err(|error| {
        io_error(
            "project_editor_composition.build_root_unavailable",
            "validate_request",
            error,
            build_root,
        )
    })?;
    let build = canonical_regular_directory(build_root, "composition build root")?;
    if build.starts_with(project_root) || project_root.starts_with(&build) {
        return Err(CompositionBuildError::new(
            "project_editor_composition.build_root_scope_rejected",
            "validate_request",
            "Composition build root and project root must be disjoint.",
            Some(&build),
            "Choose an application-owned composition build root.",
        ));
    }
    Ok(build)
}

fn ensure_owned_directory(path: &Path) -> Result<(), CompositionBuildError> {
    fs::create_dir_all(path).map_err(|error| {
        io_error(
            "project_editor_composition.owned_root_unavailable",
            "prepare_root",
            error,
            path,
        )
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        io_error(
            "project_editor_composition.owned_root_unavailable",
            "prepare_root",
            error,
            path,
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(cache_invalid(
            "Composition owned root must be a regular directory.",
            path,
        ));
    }
    Ok(())
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, CompositionBuildError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        io_error(
            "project_editor_composition.path_invalid",
            "validate_request",
            error,
            path,
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(CompositionBuildError::new(
            "project_editor_composition.path_invalid",
            "validate_request",
            format!("{label} must be a regular directory."),
            Some(path),
            "Use a regular application-owned directory.",
        ));
    }
    path.canonicalize()
        .map(normalize_windows_verbatim_path)
        .map_err(|error| {
            io_error(
                "project_editor_composition.path_invalid",
                "validate_request",
                error,
                path,
            )
        })
}

#[cfg(windows)]
fn normalize_windows_verbatim_path(path: PathBuf) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = value.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

#[cfg(not(windows))]
fn normalize_windows_verbatim_path(path: PathBuf) -> PathBuf {
    path
}

pub(crate) fn directory_size(root: &Path) -> Result<u64, CompositionBuildError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        io_error(
            "project_editor_composition.cache_measure_failed",
            "cache_prune",
            error,
            root,
        )
    })?;
    if is_link_or_reparse(&metadata) {
        return Err(cache_invalid(
            "Composition cache size cannot follow links.",
            root,
        ));
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut size = 0_u64;
    for entry in fs::read_dir(root).map_err(|error| {
        io_error(
            "project_editor_composition.cache_measure_failed",
            "cache_prune",
            error,
            root,
        )
    })? {
        let path = entry
            .map_err(|error| {
                io_error(
                    "project_editor_composition.cache_measure_failed",
                    "cache_prune",
                    error,
                    root,
                )
            })?
            .path();
        size = size.saturating_add(directory_size(&path)?);
    }
    Ok(size)
}

pub(crate) fn atomic_write_json(
    path: &Path,
    value: &impl Serialize,
) -> Result<(), CompositionBuildError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.report_encode_failed",
            "write_report",
            error.to_string(),
            Some(path),
            "Repair the typed report contract.",
        )
    })?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CompositionBuildError> {
    let parent = path
        .parent()
        .ok_or_else(|| cache_invalid("Atomic write path has no parent.", path))?;
    fs::create_dir_all(parent).map_err(|error| {
        io_error(
            "project_editor_composition.atomic_write_failed",
            "write_report",
            error,
            parent,
        )
    })?;
    let temp = path.with_extension(format!(
        "tmp-{}",
        BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temp, bytes).map_err(|error| {
        io_error(
            "project_editor_composition.atomic_write_failed",
            "write_report",
            error,
            &temp,
        )
    })?;
    fs::rename(&temp, path).map_err(|error| {
        io_error(
            "project_editor_composition.atomic_write_failed",
            "write_report",
            error,
            path,
        )
    })
}

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, CompositionBuildError> {
    let bytes = fs::read(path).map_err(|error| {
        io_error(
            "project_editor_composition.cache_invalid",
            "cache_lookup",
            error,
            path,
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.cache_invalid",
            "cache_lookup",
            error.to_string(),
            Some(path),
            "Rebuild the cached composition artifact.",
        )
    })
}

fn write_failure_report(
    build_root: &Path,
    report: &ProjectEditorCompositionBuildReport,
) -> Result<(), CompositionBuildError> {
    let reports = build_root.join(CACHE_ROOT_NAME).join("reports");
    fs::create_dir_all(&reports).map_err(|error| {
        io_error(
            "project_editor_composition.report_write_failed",
            "write_report",
            error,
            &reports,
        )
    })?;
    atomic_write_json(
        &reports.join(format!(
            "failed-{}-{}.json",
            now_epoch_seconds(),
            BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )),
        report,
    )
}

fn failure_report_is_allowed(project_root: &Path, build_root: &Path) -> bool {
    let Ok(project) = project_root
        .canonicalize()
        .map(normalize_windows_verbatim_path)
    else {
        return false;
    };
    let Ok(build) = build_root
        .canonicalize()
        .map(normalize_windows_verbatim_path)
    else {
        return false;
    };
    let Ok(metadata) = fs::symlink_metadata(build_root) else {
        return false;
    };
    metadata.is_dir()
        && !is_link_or_reparse(&metadata)
        && !build.starts_with(&project)
        && !project.starts_with(&build)
}

fn generated_executable(
    target_root: &Path,
    identity: &ProjectEditorCompositionIdentity,
) -> Result<PathBuf, CompositionBuildError> {
    Ok(target_root
        .join("release")
        .join(generated_artifact_executable_name(identity)?))
}

pub(crate) fn generated_artifact_executable_name(
    identity: &ProjectEditorCompositionIdentity,
) -> Result<String, CompositionBuildError> {
    Ok(format!(
        "{}{}",
        generated_package_name(identity)?,
        std::env::consts::EXE_SUFFIX
    ))
}

fn generated_package_name(
    identity: &ProjectEditorCompositionIdentity,
) -> Result<String, CompositionBuildError> {
    let digest = identity.digest().map_err(|error| {
        CompositionBuildError::new(
            "project_editor_composition.identity_digest_failed",
            "generate_source",
            error.to_string(),
            None,
            "Regenerate the composition identity.",
        )
    })?;
    Ok(format!(
        "aife_project_editor_{}",
        &digest.trim_start_matches("sha256:")[..16]
    ))
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cache_invalid(message: &str, path: &Path) -> CompositionBuildError {
    CompositionBuildError::new(
        "project_editor_composition.cache_invalid",
        "cache_lookup",
        message,
        Some(path),
        "Rebuild the cached composition artifact.",
    )
}

fn io_error(code: &str, stage: &str, error: std::io::Error, path: &Path) -> CompositionBuildError {
    CompositionBuildError::new(
        code,
        stage,
        error.to_string(),
        Some(path),
        "Inspect the controlled path and retry.",
    )
}

#[cfg(windows)]
pub(crate) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub(crate) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProjectEditorCompositionCachePolicy,
        PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
        PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
        PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
    };
    use std::process::Command;

    const FIXTURE_AOT_HEX: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct Fixture {
        root: PathBuf,
        project: PathBuf,
        sdk: PathBuf,
        request: ProjectEditorCompositionBuildRequest,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(label: &str) -> Fixture {
        let root = temp_root(label);
        let project = root.join("project");
        let runtime = project.join("RuntimeModule");
        fs::create_dir_all(runtime.join("src")).unwrap();
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let engine_runtime = sdk
            .join("crates/engine_runtime")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        fs::write(
            runtime.join("Cargo.toml"),
            format!(
                "[package]\nname='fixture_editor_runtime'\nversion='0.0.2'\nedition='2021'\npublish=false\n\n[dependencies]\nengine_runtime={{path='{engine_runtime}'}}\n"
            ),
        )
        .unwrap();
        fs::write(
            runtime.join("src/lib.rs"),
            format!(
                r#"use engine_runtime::project_runtime_module::{{
    EmptyProjectRuntimeModule, LinkedProjectRuntimeSet, ProjectRuntimeError,
    ProjectRuntimeModule, ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration,
}};
use std::sync::{{Arc, OnceLock}};

pub struct FixtureProjectRuntimeModule;

impl ProjectRuntimeModule for FixtureProjectRuntimeModule {{
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {{
        static DESCRIPTOR: OnceLock<ProjectRuntimeModuleDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(|| ProjectRuntimeModuleDescriptor::new(
            "fixture.editor.runtime",
            "sha256:{FIXTURE_AOT_HEX}",
        ))
    }}

    fn install(&self, registration: &mut ProjectRuntimeRegistration) -> Result<(), ProjectRuntimeError> {{
        EmptyProjectRuntimeModule::new().install(registration)
    }}
}}

pub fn linked_set() -> Result<LinkedProjectRuntimeSet, ProjectRuntimeError> {{
    LinkedProjectRuntimeSet::singleton(Arc::new(FixtureProjectRuntimeModule))
}}
"#
            ),
        )
        .unwrap();
        fs::write(
            project.join("project.aife.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "aife-project.v2",
                "projectId": "fixture.editor.project",
                "projectName": "Fixture Editor",
                "engineVersion": "0.0.2",
                "createdAt": "0",
                "lastOpenedAt": null,
                "defaultScene": "Scenes/Main.scene.json",
                "assetRoot": "Assets",
                "settingsVersion": "aife-project-settings.v1",
                "runtimeModule": {
                    "sourceKind": "projectRust",
                    "moduleId": "fixture.editor.runtime",
                    "interfaceVersion": "project-runtime-module.v2",
                    "cargoManifest": "RuntimeModule/Cargo.toml",
                    "cargoPackage": "fixture_editor_runtime",
                    "playerBinary": "fixture_editor_player"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ProjectRuntimeProductionStaging::plan(&project, &sdk).unwrap();
        let identity = ProjectEditorCompositionIdentity {
            schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: plan.manifest.project_id.clone(),
            module_id: plan.manifest.runtime_module.module_id.clone(),
            interface_version: plan.manifest.runtime_module.interface_version.clone(),
            aot_content_digest: format!("sha256:{FIXTURE_AOT_HEX}"),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc-fixture".to_string(),
            target_triple: "fixture-target".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: plan.normalized_manifest_digest,
            normalized_dependency_digest: plan.normalized_dependency_digest,
            dependency_lock_digest: plan.trusted_lock_digest,
        };
        let request = ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: project.clone(),
            engine_sdk_root: sdk.clone(),
            build_root: root.join("build"),
            expected_identity: identity,
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: crate::ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: crate::ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo-fixture".to_string(),
            capture_limit_bytes: 128 * 1024,
        };
        Fixture {
            root,
            project,
            sdk,
            request,
        }
    }

    #[test]
    fn project_editor_composition_generated_source_is_deterministic_and_project_agnostic() {
        let fixture = fixture("generated-source");
        let plan = ProjectRuntimeProductionStaging::plan(&fixture.project, &fixture.sdk).unwrap();
        let left = fixture.root.join("left");
        let right = fixture.root.join("right");
        ProjectRuntimeProductionStaging::stage(&fixture.project, &left, &plan).unwrap();
        ProjectRuntimeProductionStaging::stage(&fixture.project, &right, &plan).unwrap();
        write_generated_composition(
            &left,
            &fixture.sdk,
            &plan.manifest.runtime_module.cargo_package,
            &fixture.request.expected_identity,
        )
        .unwrap();
        write_generated_composition(
            &right,
            &fixture.sdk,
            &plan.manifest.runtime_module.cargo_package,
            &fixture.request.expected_identity,
        )
        .unwrap();
        assert_eq!(
            fs::read(left.join("GeneratedEditor/src/main.rs")).unwrap(),
            fs::read(right.join("GeneratedEditor/src/main.rs")).unwrap()
        );
        let main = fs::read_to_string(left.join("GeneratedEditor/src/main.rs")).unwrap();
        assert!(main.contains("--run-production-authority-scenario"));
        assert!(main.contains("run_real_project_editor_composition_authority"));
        assert!(main.contains("scenario_path: Some(scenario_path)"));
        assert!(
            !main.contains("production authority runner must produce a report"),
            "generated source must not panic when the production authority report is missing"
        );
        assert!(
            main.contains("production_authority_report_or_fail_closed"),
            "generated source must delegate missing-report construction to the authority owner"
        );
        assert!(main.contains("outcome.production_authority_report"));
        assert!(main.contains("production_authority_report_json(&report)"));
        assert!(
            main.contains("std::process::exit(if report.status == \"passed\" { 0 } else { 1 });")
        );
        for forbidden in ["shooter", "puzzle", "tower", "fixture.editor.runtime"] {
            assert!(!main.contains(forbidden));
        }
        let cargo: toml::Value =
            toml::from_str(&fs::read_to_string(left.join("GeneratedEditor/Cargo.toml")).unwrap())
                .unwrap();
        let dependencies = cargo["dependencies"].as_table().unwrap();
        assert_eq!(
            dependencies.keys().map(String::as_str).collect::<Vec<_>>(),
            ["editor_window_winit", "engine_runtime", "project_runtime"]
        );
    }

    #[test]
    fn project_editor_composition_deadline_distinguishes_timeout_from_process_failure() {
        let mut fixture = fixture("build-failures");
        let fake = compile_fake_cargo(&fixture.root);
        for (mode, timeout, expected_reason) in [
            ("nonzero", 30_000, BoundedChildProcessExitReason::Failed),
            ("timeout", 10, BoundedChildProcessExitReason::Timeout),
            ("output", 30_000, BoundedChildProcessExitReason::Failed),
        ] {
            let executable = fixture
                .root
                .join(format!("fake-cargo-{mode}{}", std::env::consts::EXE_SUFFIX));
            fs::copy(&fake, &executable).unwrap();
            fixture.request.cargo_executable = Some(executable);
            fixture
                .request
                .deadline_policy
                .generate_lock_hard_deadline_ms = timeout;
            fixture
                .request
                .deadline_policy
                .release_build_hard_deadline_ms = timeout;
            fixture.request.deadline_policy.release_build_soft_budget_ms =
                timeout.saturating_sub(1).max(1);
            fixture.request.capture_limit_bytes = 64;
            let report = ProjectEditorCompositionArtifact::prepare(
                fixture.request.clone(),
                ProjectEditorCompositionPreparationControl::default(),
            );
            assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Failed);
            assert_eq!(report.steps.len(), 1);
            assert_eq!(report.steps[0].process.exit_reason, expected_reason);
            assert!(report.output_readers_joined);
            assert!(report.root_wait_completed);
            assert!(report.process_group_released);
            assert!(report.owned_process_cleanup_confirmed);
            if mode == "output" {
                assert!(report.steps[0].process.stderr_truncated);
                assert!(report.steps[0].process.stderr_total_bytes >= 4096);
            }
            assert_eq!(report.cleanup_status, "removed");
            let staging = fixture
                .request
                .build_root
                .join(CACHE_ROOT_NAME)
                .join("staging");
            assert_eq!(fs::read_dir(staging).unwrap().count(), 0);
        }
    }

    #[test]
    fn project_editor_composition_build_qos_applies_unique_jobs_and_priority_evidence() {
        let mut fixture = fixture("build-qos-command");
        let fake = compile_fake_cargo(&fixture.root);
        let executable = fixture
            .root
            .join(format!("fake-cargo-qos{}", std::env::consts::EXE_SUFFIX));
        fs::copy(&fake, &executable).unwrap();
        fixture.request.cargo_executable = Some(executable);

        let report = ProjectEditorCompositionArtifact::prepare(
            fixture.request.clone(),
            ProjectEditorCompositionPreparationControl::default(),
        );
        assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Failed);
        assert_eq!(report.steps.len(), 2);
        let build = &report.steps[1];
        let jobs_positions = build
            .command
            .iter()
            .enumerate()
            .filter_map(|(index, value)| (value == "--jobs").then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(jobs_positions.len(), 1);
        assert_eq!(
            build.command[jobs_positions[0] + 1],
            report
                .qos_decision
                .as_ref()
                .unwrap()
                .resolved_jobs
                .to_string()
        );
        assert_eq!(
            build.process.priority.requested,
            BoundedChildProcessPriority::BelowNormal
        );
        if cfg!(windows) {
            assert!(build.process.priority.applied);
        }
    }

    #[test]
    fn project_editor_composition_process_terminal_reports_cancel_and_cleanup_evidence() {
        let mut fixture = fixture("build-qos-cancel");
        let fake = compile_fake_cargo(&fixture.root);
        let executable = fixture.root.join(format!(
            "fake-cargo-timeout{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::copy(&fake, &executable).unwrap();
        fixture.request.cargo_executable = Some(executable);
        fixture
            .request
            .deadline_policy
            .generate_lock_hard_deadline_ms = 30_000;
        fixture.request.deadline_policy.release_build_soft_budget_ms = 29_999;
        fixture
            .request
            .deadline_policy
            .release_build_hard_deadline_ms = 30_000;
        let control = ProjectEditorCompositionPreparationControl::default();
        let signal = control.clone();
        let cancel = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            signal.request_cancel();
        });

        let report = ProjectEditorCompositionArtifact::prepare(fixture.request.clone(), control);
        cancel.join().unwrap();
        assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Failed);
        assert_eq!(
            report.steps[0].process.exit_reason,
            BoundedChildProcessExitReason::Cancelled
        );
        assert!(report.cancellation_requested);
        assert!(report.process_tree_terminated);
        assert!(report.output_readers_joined);
        assert!(report.root_wait_completed);
        assert!(report.process_group_released);
        assert!(report.owned_process_cleanup_confirmed);
        assert!(report.steps[0].process.owned_process_cleanup_confirmed());
        assert_eq!(report.cleanup_status, "removed");
        assert_eq!(
            report.diagnostics[0].code,
            "project_editor_composition.cancelled"
        );
    }

    #[test]
    fn project_editor_composition_build_qos_reports_descriptor_query_cancellation() {
        let fixture = fixture("descriptor-query-cancel");
        let fake = compile_fake_cargo(&fixture.root);
        let executable = fixture.root.join(format!(
            "fake-cargo-timeout{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::copy(fake, &executable).unwrap();
        let mut report: ProjectEditorCompositionBuildReport =
            serde_json::from_value(serde_json::json!({
                "schemaVersion": PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
                "status": "failed",
                "cacheStatus": "not_checked",
                "cleanupStatus": "not_required",
                "steps": [],
                "diagnostics": []
            }))
            .unwrap();
        let control = ProjectEditorCompositionPreparationControl::default();
        control.request_cancel();

        let error =
            query_descriptor(&executable, &mut report, &fixture.request, &control).unwrap_err();

        assert_eq!(error.code, "project_editor_composition.cancelled");
        assert!(report.cancellation_requested);
        assert!(report.process_tree_terminated);
        assert_eq!(
            report.steps[0].process.exit_reason,
            BoundedChildProcessExitReason::Cancelled
        );
    }

    #[test]
    fn project_editor_composition_cache_identity_key_tracks_every_domain() {
        let baseline = fixture("cache-key").request.expected_identity.clone();
        let digest = baseline.digest().unwrap();
        let mutations: Vec<Box<dyn Fn(&mut ProjectEditorCompositionIdentity)>> = vec![
            Box::new(|value| value.project_id.push('x')),
            Box::new(|value| value.module_id.push('x')),
            Box::new(|value| value.interface_version.push('x')),
            Box::new(|value| value.aot_content_digest.replace_range(7..8, "b")),
            Box::new(|value| value.editor_build_identity.replace_range(7..8, "c")),
            Box::new(|value| value.engine_sdk_digest.replace_range(7..8, "d")),
            Box::new(|value| value.toolchain_identity.push('x')),
            Box::new(|value| value.target_triple.push('x')),
            Box::new(|value| value.profile.push('x')),
            Box::new(|value| value.normalized_manifest_digest.push('x')),
            Box::new(|value| value.normalized_dependency_digest.push('x')),
            Box::new(|value| value.dependency_lock_digest.push('x')),
        ];
        for mutate in mutations {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(changed.digest().unwrap(), digest);
        }
        let mut other_project = baseline.clone();
        other_project.project_id = "fixture.other.project".to_string();
        let other_digest = other_project.digest().unwrap();
        assert_ne!(digest, other_digest);
        assert_ne!(
            &digest.trim_start_matches("sha256:")[..16],
            &other_digest.trim_start_matches("sha256:")[..16]
        );
    }

    #[test]
    fn project_editor_composition_compilation_cache_v2_enforces_path_affinity() {
        let fixture = fixture("compilation-cache-key");
        let cache_root = fixture.request.build_root.join(CACHE_ROOT_NAME);
        fs::create_dir_all(&cache_root).unwrap();
        let lineage = GeneratedCompositionLockLineage {
            schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
            lock_input_digest: format!("sha256:{}", "1".repeat(64)),
            raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
            resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
        };
        let empty_report = || {
            serde_json::from_value::<ProjectEditorCompositionBuildReport>(serde_json::json!({
                "schemaVersion": PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
                "status": "failed",
                "cacheStatus": "not_checked",
                "cleanupStatus": "not_required",
                "steps": [],
                "diagnostics": []
            }))
            .unwrap()
        };
        let compatibility =
            composition_compilation_compatibility_digest(&fixture.request, &lineage);
        let mut cold_report = empty_report();
        let target = prepare_composition_compilation_target_root(
            &cache_root,
            &fixture.request,
            &lineage,
            &mut cold_report,
        )
        .unwrap();
        assert!(target.starts_with(cache_root.join("ct")));
        assert!(target.join("affinity.json").is_file());
        assert_eq!(
            cold_report.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::Cold
        );
        assert_eq!(
            cold_report.compilation_cache_compatibility_digest,
            Some(compatibility)
        );
        assert!(!cold_report.cross_root_portable);

        let mut hit_report = empty_report();
        assert_eq!(
            prepare_composition_compilation_target_root(
                &cache_root,
                &fixture.request,
                &lineage,
                &mut hit_report,
            )
            .unwrap(),
            target
        );
        assert_eq!(
            hit_report.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::SameRootHit
        );

        fs::remove_file(target.join("affinity.json")).unwrap();
        let mut copied_report = empty_report();
        assert_eq!(
            prepare_composition_compilation_target_root(
                &cache_root,
                &fixture.request,
                &lineage,
                &mut copied_report,
            )
            .unwrap(),
            target
        );
        assert_eq!(
            copied_report.compilation_cache_affinity,
            ProjectEditorCompositionCompilationCacheAffinity::PathAffineMiss
        );
    }

    #[test]
    fn project_editor_composition_lineage_store_hits_and_rejects_tamper() {
        let fixture = fixture("lineage-store");
        let cache_root = fixture.request.build_root.join(CACHE_ROOT_NAME);
        let generated_root = fixture.root.join("generated-lineage");
        fs::create_dir_all(cache_root.join("locks")).unwrap();
        fs::create_dir_all(&generated_root).unwrap();
        let input_digest = format!("sha256:{}", "1".repeat(64));
        let root_name = generated_package_name(&fixture.request.expected_identity).unwrap();
        let lock = format!(
            "version = 3\n\n[[package]]\nname = \"{root_name}\"\nversion = \"0.0.2\"\n\n[[package]]\nname = \"fixture_dep\"\nversion = \"1.0.0\"\n"
        );
        let lineage =
            generated_composition_lock_lineage(lock.as_bytes(), &root_name, input_digest.clone())
                .unwrap();
        let lineage_root = cache_root
            .join("locks")
            .join(input_digest.trim_start_matches("sha256:"));
        fs::create_dir_all(&lineage_root).unwrap();
        fs::write(lineage_root.join("Cargo.lock"), lock.as_bytes()).unwrap();
        atomic_write_json(&lineage_root.join("lineage.json"), &lineage).unwrap();
        let mut report: ProjectEditorCompositionBuildReport =
            serde_json::from_value(serde_json::json!({
                "schemaVersion": PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
                "status": "failed",
                "cacheStatus": "not_checked",
                "cleanupStatus": "not_required",
                "steps": [],
                "diagnostics": []
            }))
            .unwrap();
        let hit = prepare_generated_lock_lineage(
            &cache_root,
            &generated_root,
            Path::new("must-not-run-cargo"),
            &fixture.request,
            &ProjectEditorCompositionPreparationControl::default(),
            &mut report,
            input_digest.clone(),
        )
        .unwrap();
        assert_eq!(hit, lineage);
        assert!(report.steps.is_empty());
        assert_eq!(
            fs::read(generated_root.join("Cargo.lock")).unwrap(),
            lock.as_bytes()
        );

        fs::write(
            lineage_root.join("Cargo.lock"),
            lock.replace("1.0.0", "1.0.1"),
        )
        .unwrap();
        let error = prepare_generated_lock_lineage(
            &cache_root,
            &generated_root,
            Path::new("must-not-run-cargo"),
            &fixture.request,
            &ProjectEditorCompositionPreparationControl::default(),
            &mut report,
            input_digest,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "project_editor_composition.lock_lineage_raw_digest_mismatch"
        );
    }

    #[test]
    fn project_editor_composition_lineage_store_manifest_template_is_path_independent() {
        let left = br#"[package]
name = "generated"
version = "0.0.2"

[dependencies]
project_runtime = { path = "G:/run-a/RuntimeModuleBuild", package = "fixture" }
"#;
        let right = br#"[package]
name = "generated"
version = "0.0.2"

[dependencies]
project_runtime = { path = "G:/run-b/RuntimeModuleBuild", package = "fixture" }
"#;
        assert_eq!(
            canonical_generated_manifest_template_digest(left).unwrap(),
            canonical_generated_manifest_template_digest(right).unwrap()
        );
        let changed = br#"[package]
name = "generated"
version = "0.0.2"

[dependencies]
project_runtime = { path = "G:/run-b/RuntimeModuleBuild", package = "fixture", features = ["extra"] }
"#;
        assert_ne!(
            canonical_generated_manifest_template_digest(left).unwrap(),
            canonical_generated_manifest_template_digest(changed).unwrap()
        );
    }

    #[test]
    fn project_editor_composition_cache_invalidates_descriptor_and_executable_tamper() {
        let fixture = fixture("cache-tamper");
        let artifact_root = fixture.root.join("artifact");
        fs::create_dir_all(artifact_root.join("bin")).unwrap();
        let executable = artifact_root
            .join("bin")
            .join(generated_artifact_executable_name(&fixture.request.expected_identity).unwrap());
        fs::write(&executable, b"sealed").unwrap();
        fs::write(artifact_root.join("build-report.json"), b"{}").unwrap();
        let identity = fixture.request.expected_identity.clone();
        let identity_digest = identity.digest().unwrap();
        let resolved_identity = ProjectEditorCompositionResolvedIdentity::new(
            identity_digest.clone(),
            &GeneratedCompositionLockLineage {
                schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
                lock_input_digest: format!("sha256:{}", "1".repeat(64)),
                raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
                resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
            },
        )
        .unwrap();
        atomic_write_json(
            &artifact_root.join("composition-descriptor.json"),
            &ProjectEditorCompositionDescriptor {
                schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
                identity: identity.clone(),
                identity_digest: identity_digest.clone(),
                resolved_identity: resolved_identity.clone(),
                executable_hash: sha256_prefixed(b"sealed"),
                created_at: 1,
            },
        )
        .unwrap();
        assert!(load_cached_artifact(
            &artifact_root,
            &identity,
            &identity_digest,
            &resolved_identity,
        )
        .is_ok());
        let mut descriptor: ProjectEditorCompositionDescriptor =
            read_json(&artifact_root.join("composition-descriptor.json")).unwrap();
        descriptor.identity.module_id = "fixture.tampered.runtime".to_string();
        atomic_write_json(
            &artifact_root.join("composition-descriptor.json"),
            &descriptor,
        )
        .unwrap();
        assert_eq!(
            load_cached_artifact(
                &artifact_root,
                &identity,
                &identity_digest,
                &resolved_identity
            )
            .unwrap_err()
            .code,
            "project_editor_composition.cache_invalid"
        );
        descriptor.identity = identity.clone();
        atomic_write_json(
            &artifact_root.join("composition-descriptor.json"),
            &descriptor,
        )
        .unwrap();
        fs::write(&executable, b"tampered").unwrap();
        assert_eq!(
            load_cached_artifact(
                &artifact_root,
                &identity,
                &identity_digest,
                &resolved_identity
            )
            .unwrap_err()
            .code,
            "project_editor_composition.cache_invalid"
        );
    }

    #[test]
    fn project_editor_composition_cache_prunes_lru_preserves_pin_and_fails_closed_at_hard_limit() {
        let root = temp_root("cache-lru");
        let cache_root = root.join(CACHE_ROOT_NAME);
        fs::create_dir_all(cache_root.join("cache")).unwrap();
        fs::create_dir_all(cache_root.join("pins")).unwrap();
        for (key, last_used, pinned) in [("old", 1, false), ("active", 2, true)] {
            let path = cache_root.join("cache").join(key);
            fs::create_dir_all(&path).unwrap();
            atomic_write_json(
                &path.join("cache-entry.json"),
                &CacheEntryMetadata {
                    schema_version: "project-editor-composition-cache-entry.v1".to_string(),
                    identity_digest: format!("sha256:{key}"),
                    project_id: "fixture.project".to_string(),
                    size_bytes: 40,
                    last_used_at: last_used,
                },
            )
            .unwrap();
            if pinned {
                fs::write(cache_root.join("pins").join(format!("{key}.pin")), b"").unwrap();
            }
        }
        let policy = ProjectEditorCompositionCachePolicy {
            global_soft_limit_bytes: 80,
            global_hard_limit_bytes: 100,
            per_project_soft_limit_bytes: 80,
            per_project_hard_limit_bytes: 100,
        };
        ensure_cache_capacity(&cache_root, "fixture.project", "incoming", 40, &policy).unwrap();
        assert!(!cache_root.join("cache/old").exists());
        assert!(cache_root.join("cache/active").exists());
        assert_eq!(
            ensure_cache_capacity(&cache_root, "fixture.project", "incoming", 70, &policy)
                .unwrap_err()
                .code,
            "project_editor_composition.cache_capacity_exceeded"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_editor_composition_cleanup_rejects_reparse_escape() {
        let root = temp_root("cleanup-reparse");
        let cache_root = root.join(CACHE_ROOT_NAME);
        let external = root.join("external");
        let linked = cache_root.join("cache/linked");
        fs::create_dir_all(cache_root.join("cache")).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("keep.txt"), b"keep").unwrap();
        create_directory_link(&linked, &external);
        assert_eq!(
            cleanup_owned_directory(&root, &linked).unwrap_err(),
            "retained_by_host_policy"
        );
        assert_eq!(
            remove_owned_cache_entry(&cache_root, &linked)
                .unwrap_err()
                .code,
            "project_editor_composition.cleanup_scope_rejected"
        );
        assert_eq!(fs::read(external.join("keep.txt")).unwrap(), b"keep");
        remove_directory_link(&linked);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_editor_composition_cleanup_invalid_build_root_never_writes_project() {
        let mut fixture = fixture("invalid-build-root");
        fixture.request.build_root = fixture.project.join("forged-build-root");
        let report = ProjectEditorCompositionArtifact::prepare(
            fixture.request.clone(),
            ProjectEditorCompositionPreparationControl::default(),
        );
        assert_eq!(report.status, ProjectEditorCompositionBuildStatus::Failed);
        assert_eq!(
            report.diagnostics[0].code,
            "project_editor_composition.build_root_scope_rejected"
        );
        assert!(!fixture.request.build_root.exists());
    }

    fn compile_fake_cargo(root: &Path) -> PathBuf {
        let source = root.join("fake_cargo.rs");
        let executable = root.join(format!("fake-cargo{}", std::env::consts::EXE_SUFFIX));
        fs::write(
            &source,
            r#"fn main() {
    let name = std::env::current_exe().unwrap().file_stem().unwrap().to_string_lossy().to_string();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if name.contains("qos") && args.first().is_some_and(|arg| arg == "generate-lockfile") {
        std::fs::write("Cargo.lock", "version = 3\n\n[[package]]\nname = \"fixture_dep\"\nversion = \"1.0.0\"\n").unwrap();
        return;
    }
    if name.contains("qos") { eprintln!("{}", args.join("|")); std::process::exit(7); }
    if name.contains("timeout") { std::thread::sleep(std::time::Duration::from_secs(5)); return; }
    if name.contains("output") { eprintln!("{}", "x".repeat(4096)); std::process::exit(9); }
    eprintln!("intentional non-zero");
    std::process::exit(7);
}
"#,
        )
        .unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
        let output = Command::new(rustc)
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fake cargo compile failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        executable
    }

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        if cfg!(windows) {
            PathBuf::from("G:/Aife262W2").join(format!("{label}-{}-{stamp}", std::process::id()))
        } else {
            std::env::temp_dir().join(format!(
                "aife-262-window2-{label}-{}-{stamp}",
                std::process::id()
            ))
        }
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let link = link.display().to_string().replace('/', "\\");
        let target = target.display().to_string().replace('/', "\\");
        let output = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction creation failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(not(windows))]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).unwrap();
    }

    #[cfg(not(windows))]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).unwrap();
    }
}
