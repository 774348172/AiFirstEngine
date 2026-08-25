use engine_runtime::canonical_digest::{sha256_prefixed, CanonicalDigestError, ConsistencyDigest};
use runtime_cli::BoundedChildProcessResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub const PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-build-request.v1";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION_V2: &str =
    "project-editor-composition-build-request.v2";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION: &str =
    "project-editor-composition-build-request.v3";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_QOS_POLICY_SCHEMA_VERSION: &str =
    "project-editor-composition-build-qos-policy.v1";
pub const PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION: &str =
    "project-editor-composition-artifact.v2";
pub const PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-artifact.v1";
pub const PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION: &str =
    "project-editor-composition-descriptor.v2";
pub const PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-descriptor.v1";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-build-report.v1";
pub const PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION: &str =
    "project-editor-composition-qualification-seal.v2";
pub const PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-qualification-seal.v1";
pub const PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION: &str =
    "project-editor-composition-promotion-request.v2";
pub const PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-promotion-request.v1";
pub const PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION: &str =
    "project-editor-composition-promotion-report.v2";
pub const PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION_V1: &str =
    "project-editor-composition-promotion-report.v1";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V2: &str =
    "project-editor-composition-build-report.v2";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION: &str =
    "project-editor-composition-build-report.v3";
pub const GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION: &str =
    "generated-composition-lock-lineage.v1";
pub const PROJECT_EDITOR_COMPOSITION_RESOLVED_IDENTITY_SCHEMA_VERSION: &str =
    "project-editor-composition-resolved-identity.v1";
pub const PROJECT_EDITOR_COMPOSITION_BUILD_DEADLINE_POLICY_SCHEMA_VERSION: &str =
    "project-editor-composition-build-deadline-policy.v1";
pub const PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION: &str =
    "project-editor-composition-handoff-ticket.v1";
pub const PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION: &str =
    "project-editor-composition-launch-receipt.v1";
pub const PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION: &str =
    "project-editor-composition-identity.v1";
const DEFAULT_COMPOSITION_CAPTURE_LIMIT_BYTES: usize = 128 * 1024;
const GIBIBYTE: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildDeadlinePolicy {
    pub schema_version: String,
    pub generate_lock_hard_deadline_ms: u64,
    pub release_build_soft_budget_ms: u64,
    pub release_build_hard_deadline_ms: u64,
    pub descriptor_query_hard_deadline_ms: u64,
}

impl Default for ProjectEditorCompositionBuildDeadlinePolicy {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_DEADLINE_POLICY_SCHEMA_VERSION
                .to_string(),
            generate_lock_hard_deadline_ms: 60_000,
            release_build_soft_budget_ms: 600_000,
            release_build_hard_deadline_ms: 1_200_000,
            descriptor_query_hard_deadline_ms: 30_000,
        }
    }
}

impl ProjectEditorCompositionBuildDeadlinePolicy {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_BUILD_DEADLINE_POLICY_SCHEMA_VERSION
            || self.generate_lock_hard_deadline_ms == 0
            || self.release_build_soft_budget_ms == 0
            || self.release_build_hard_deadline_ms == 0
            || self.descriptor_query_hard_deadline_ms == 0
            || self.release_build_soft_budget_ms >= self.release_build_hard_deadline_ms
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.deadline_policy_invalid",
                "Composition deadlines must use the supported schema and ordered non-zero release budgets.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedCompositionLockInput {
    pub schema_version: String,
    pub cargo_identity: String,
    pub toolchain_identity: String,
    pub target_triple: String,
    pub profile: String,
    pub generated_feature_set: Vec<String>,
    pub generated_manifest_template_digest: String,
    pub runtime_module_manifest_digest: String,
    pub normalized_dependency_identity_digest: String,
    pub engine_sdk_lock_digest: String,
    pub trusted_engine_manifest_set_digest: String,
}

impl GeneratedCompositionLockInput {
    pub fn digest(&self) -> Result<String, ProjectEditorCompositionContractError> {
        if self.schema_version != GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION
            || self.cargo_identity.trim().is_empty()
            || self.toolchain_identity.trim().is_empty()
            || self.target_triple.trim().is_empty()
            || self.profile.trim().is_empty()
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_input_mismatch",
                "Generated lock input must use the supported schema and non-empty build identities.",
            ));
        }
        for digest in [
            &self.generated_manifest_template_digest,
            &self.runtime_module_manifest_digest,
            &self.normalized_dependency_identity_digest,
            &self.engine_sdk_lock_digest,
            &self.trusted_engine_manifest_set_digest,
        ] {
            if !is_sha256_identity(digest) {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.lock_lineage_input_mismatch",
                    "Generated lock input digests must be canonical SHA-256 values.",
                ));
            }
        }
        ConsistencyDigest::sha256(
            "generated-composition-lock-input",
            GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
            self,
        )
        .map(|digest| digest.prefixed_value())
        .map_err(|error| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_input_mismatch",
                error.to_string(),
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedCompositionLockLineage {
    pub schema_version: String,
    pub lock_input_digest: String,
    pub raw_lock_digest: String,
    pub resolved_graph_digest: String,
}

impl GeneratedCompositionLockLineage {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION
            || !is_sha256_identity(&self.lock_input_digest)
            || !is_sha256_identity(&self.raw_lock_digest)
            || !is_sha256_identity(&self.resolved_graph_digest)
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_input_mismatch",
                "Generated lock lineage must use the supported schema and canonical SHA-256 digests.",
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, ProjectEditorCompositionContractError> {
        self.validate()?;
        ConsistencyDigest::sha256(
            "generated-composition-lock-lineage",
            GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
            self,
        )
        .map(|digest| digest.prefixed_value())
        .map_err(|error| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_input_mismatch",
                error.to_string(),
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalLockedPackage {
    name: String,
    version: String,
    source: String,
    checksum: Option<String>,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalResolvedGraph {
    packages: Vec<CanonicalLockedPackage>,
}

pub fn generated_composition_lock_lineage(
    raw_lock: &[u8],
    generated_root_package_name: &str,
    lock_input_digest: String,
) -> Result<GeneratedCompositionLockLineage, ProjectEditorCompositionContractError> {
    if !is_sha256_identity(&lock_input_digest) || generated_root_package_name.trim().is_empty() {
        return Err(ProjectEditorCompositionContractError::new(
            "project_editor_composition.lock_lineage_input_mismatch",
            "Generated lock lineage requires one canonical input digest and root package name.",
        ));
    }
    let text = std::str::from_utf8(raw_lock).map_err(|error| {
        ProjectEditorCompositionContractError::new(
            "project_editor_composition.lock_lineage_graph_digest_mismatch",
            format!("Generated Cargo.lock is not UTF-8: {error}"),
        )
    })?;
    let lock: toml::Value = toml::from_str(text).map_err(|error| {
        ProjectEditorCompositionContractError::new(
            "project_editor_composition.lock_lineage_graph_digest_mismatch",
            format!("Generated Cargo.lock is invalid TOML: {error}"),
        )
    })?;
    let package_values = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_graph_digest_mismatch",
                "Generated Cargo.lock must contain a package array.",
            )
        })?;
    let mut packages = Vec::new();
    for value in package_values {
        let table = value.as_table().ok_or_else(|| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_graph_digest_mismatch",
                "Generated Cargo.lock package entries must be tables.",
            )
        })?;
        let name = required_lock_string(table, "name")?;
        let version = required_lock_string(table, "version")?;
        if name == generated_root_package_name && table.get("source").is_none() {
            continue;
        }
        let source = match table.get("source").and_then(toml::Value::as_str) {
            None => "path".to_string(),
            Some(value) if value.starts_with("registry+") || value.starts_with("git+") => {
                value.to_string()
            }
            Some(value) => {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.lock_lineage_graph_digest_mismatch",
                    format!("Unsupported Cargo.lock source '{value}'."),
                ));
            }
        };
        let checksum = table
            .get("checksum")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
        if source != "path" && checksum.is_none() {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_graph_digest_mismatch",
                format!("Locked package '{name} {version}' is missing its checksum."),
            ));
        }
        let mut dependencies = table
            .get("dependencies")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .map(|dependency| {
                dependency.as_str().map(str::to_string).ok_or_else(|| {
                    ProjectEditorCompositionContractError::new(
                        "project_editor_composition.lock_lineage_graph_digest_mismatch",
                        "Cargo.lock dependency edges must be strings.",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        dependencies.sort();
        dependencies.dedup();
        packages.push(CanonicalLockedPackage {
            name,
            version,
            source,
            checksum,
            dependencies,
        });
    }
    packages.sort_by(|left, right| {
        (&left.name, &left.version, &left.source).cmp(&(&right.name, &right.version, &right.source))
    });
    if packages.windows(2).any(|pair| {
        pair[0].name == pair[1].name
            && pair[0].version == pair[1].version
            && pair[0].source == pair[1].source
    }) {
        return Err(ProjectEditorCompositionContractError::new(
            "project_editor_composition.lock_lineage_graph_digest_mismatch",
            "Generated Cargo.lock contains ambiguous duplicate package identities.",
        ));
    }
    let resolved_graph_digest = ConsistencyDigest::sha256(
        "generated-composition-resolved-graph",
        GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
        &CanonicalResolvedGraph { packages },
    )
    .map(|digest| digest.prefixed_value())
    .map_err(|error| {
        ProjectEditorCompositionContractError::new(
            "project_editor_composition.lock_lineage_graph_digest_mismatch",
            error.to_string(),
        )
    })?;
    let lineage = GeneratedCompositionLockLineage {
        schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
        lock_input_digest,
        raw_lock_digest: sha256_prefixed(raw_lock),
        resolved_graph_digest,
    };
    lineage.validate()?;
    Ok(lineage)
}

fn required_lock_string(
    table: &toml::map::Map<String, toml::Value>,
    field: &str,
) -> Result<String, ProjectEditorCompositionContractError> {
    table
        .get(field)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.lock_lineage_graph_digest_mismatch",
                format!("Cargo.lock package field '{field}' is required."),
            )
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionResolvedIdentity {
    pub schema_version: String,
    pub requested_identity_digest: String,
    pub lock_lineage_digest: String,
    pub resolved_graph_digest: String,
    pub resolved_artifact_key_digest: String,
}

impl ProjectEditorCompositionResolvedIdentity {
    pub fn new(
        requested_identity_digest: String,
        lineage: &GeneratedCompositionLockLineage,
    ) -> Result<Self, ProjectEditorCompositionContractError> {
        let lock_lineage_digest = lineage.digest()?;
        let resolved_artifact_key_digest = ConsistencyDigest::sha256(
            "project-editor-composition-resolved-artifact-key",
            PROJECT_EDITOR_COMPOSITION_RESOLVED_IDENTITY_SCHEMA_VERSION,
            &(
                &requested_identity_digest,
                &lock_lineage_digest,
                &lineage.resolved_graph_digest,
            ),
        )
        .map(|digest| digest.prefixed_value())
        .map_err(|error| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.resolved_identity_mismatch",
                error.to_string(),
            )
        })?;
        let value = Self {
            schema_version: PROJECT_EDITOR_COMPOSITION_RESOLVED_IDENTITY_SCHEMA_VERSION.to_string(),
            requested_identity_digest,
            lock_lineage_digest,
            resolved_graph_digest: lineage.resolved_graph_digest.clone(),
            resolved_artifact_key_digest,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_RESOLVED_IDENTITY_SCHEMA_VERSION
            || !is_sha256_identity(&self.requested_identity_digest)
            || !is_sha256_identity(&self.lock_lineage_digest)
            || !is_sha256_identity(&self.resolved_graph_digest)
            || !is_sha256_identity(&self.resolved_artifact_key_digest)
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.resolved_identity_mismatch",
                "Resolved composition identity is incomplete or malformed.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildQosPolicy {
    pub schema_version: String,
    pub max_jobs: u16,
    pub reserved_logical_processors: u16,
    pub reserved_memory_bytes: u64,
    pub estimated_memory_per_job_bytes: u64,
    pub min_jobs: u16,
}

impl Default for ProjectEditorCompositionBuildQosPolicy {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_QOS_POLICY_SCHEMA_VERSION.to_string(),
            max_jobs: 4,
            reserved_logical_processors: 4,
            reserved_memory_bytes: 3 * GIBIBYTE,
            estimated_memory_per_job_bytes: GIBIBYTE,
            min_jobs: 1,
        }
    }
}

impl ProjectEditorCompositionBuildQosPolicy {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_BUILD_QOS_POLICY_SCHEMA_VERSION
            || self.max_jobs == 0
            || self.min_jobs == 0
            || self.min_jobs > self.max_jobs
            || self.estimated_memory_per_job_bytes == 0
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.qos_policy_invalid",
                "Composition build QoS policy must use the supported schema and ordered non-zero job and memory limits.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionSystemFacts {
    pub logical_processors: Option<u16>,
    pub available_memory_bytes: Option<u64>,
}

impl ProjectEditorCompositionSystemFacts {
    pub fn collect() -> Self {
        Self {
            logical_processors: std::thread::available_parallelism()
                .ok()
                .and_then(|value| u16::try_from(value.get()).ok()),
            available_memory_bytes: available_memory_bytes(),
        }
    }
}

#[cfg(windows)]
fn available_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).ok()?;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        None
    } else {
        Some(status.ullAvailPhys)
    }
}

#[cfg(not(windows))]
fn available_memory_bytes() -> Option<u64> {
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildQosDecision {
    pub resolved_jobs: u16,
    pub cpu_limited_jobs: Option<u16>,
    pub memory_limited_jobs: Option<u16>,
    pub used_conservative_fallback: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectEditorCompositionPreparationControl {
    cancellation: runtime_cli::BoundedChildProcessCancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectEditorCompositionPreparationPhase {
    Inspecting,
    CacheLookup,
    Staging,
    Compiling,
    Sealing,
    Ready,
    Cancelled,
}

impl ProjectEditorCompositionPreparationControl {
    pub fn request_cancel(&self) {
        self.cancellation.request_cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub(crate) fn process_cancellation(&self) -> runtime_cli::BoundedChildProcessCancellation {
        self.cancellation.clone()
    }
}

pub fn resolve_project_editor_composition_build_qos(
    policy: &ProjectEditorCompositionBuildQosPolicy,
    facts: ProjectEditorCompositionSystemFacts,
) -> Result<ProjectEditorCompositionBuildQosDecision, ProjectEditorCompositionContractError> {
    policy.validate()?;
    let mut diagnostics = Vec::new();
    let cpu_limited_jobs = facts.logical_processors.map(|logical| {
        logical
            .saturating_sub(policy.reserved_logical_processors)
            .max(policy.min_jobs)
    });
    let memory_limited_jobs = facts.available_memory_bytes.map(|available| {
        if available <= policy.reserved_memory_bytes {
            policy.min_jobs
        } else {
            u16::try_from(
                (available - policy.reserved_memory_bytes) / policy.estimated_memory_per_job_bytes,
            )
            .unwrap_or(u16::MAX)
            .max(policy.min_jobs)
        }
    });
    let used_conservative_fallback = cpu_limited_jobs.is_none() || memory_limited_jobs.is_none();
    if used_conservative_fallback {
        diagnostics.push("project_editor_composition.qos_system_facts_unavailable".to_string());
    }
    let resolved_jobs = match (cpu_limited_jobs, memory_limited_jobs) {
        (Some(cpu), Some(memory)) => policy.max_jobs.min(cpu).min(memory),
        (Some(cpu), None) => policy.max_jobs.min(cpu),
        (None, Some(memory)) => policy.max_jobs.min(memory),
        (None, None) => policy.min_jobs,
    }
    .max(policy.min_jobs);
    Ok(ProjectEditorCompositionBuildQosDecision {
        resolved_jobs,
        cpu_limited_jobs,
        memory_limited_jobs,
        used_conservative_fallback,
        diagnostics,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionCachePolicy {
    pub global_soft_limit_bytes: u64,
    pub global_hard_limit_bytes: u64,
    pub per_project_soft_limit_bytes: u64,
    pub per_project_hard_limit_bytes: u64,
}

impl Default for ProjectEditorCompositionCachePolicy {
    fn default() -> Self {
        Self {
            global_soft_limit_bytes: 24 * 1024 * 1024 * 1024,
            global_hard_limit_bytes: 32 * 1024 * 1024 * 1024,
            per_project_soft_limit_bytes: 8 * 1024 * 1024 * 1024,
            per_project_hard_limit_bytes: 12 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionIdentity {
    pub schema_version: String,
    pub project_id: String,
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
    pub editor_build_identity: String,
    pub engine_sdk_digest: String,
    pub toolchain_identity: String,
    pub target_triple: String,
    pub profile: String,
    pub normalized_manifest_digest: String,
    pub normalized_dependency_digest: String,
    pub dependency_lock_digest: String,
}

impl ProjectEditorCompositionIdentity {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.identity_schema_unsupported",
                format!(
                    "Unsupported composition identity schema: {}",
                    self.schema_version
                ),
            ));
        }

        for (field, value) in [
            ("projectId", self.project_id.as_str()),
            ("moduleId", self.module_id.as_str()),
            ("interfaceVersion", self.interface_version.as_str()),
            ("toolchainIdentity", self.toolchain_identity.as_str()),
            ("targetTriple", self.target_triple.as_str()),
            ("profile", self.profile.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.identity_field_required",
                    format!("Composition identity field {field} cannot be empty."),
                ));
            }
        }

        for (field, value) in [
            ("aotContentDigest", self.aot_content_digest.as_str()),
            ("editorBuildIdentity", self.editor_build_identity.as_str()),
            ("engineSdkDigest", self.engine_sdk_digest.as_str()),
            (
                "normalizedManifestDigest",
                self.normalized_manifest_digest.as_str(),
            ),
            (
                "normalizedDependencyDigest",
                self.normalized_dependency_digest.as_str(),
            ),
            ("dependencyLockDigest", self.dependency_lock_digest.as_str()),
        ] {
            if !is_sha256_identity(value) {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.identity_digest_invalid",
                    format!(
                        "Composition identity field {field} must be sha256:<64 lowercase hex>."
                    ),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, CanonicalDigestError> {
        ConsistencyDigest::sha256(
            "project-editor-composition-identity",
            PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
            self,
        )
        .map(|digest| digest.prefixed_value())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildRequest {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub engine_sdk_root: PathBuf,
    pub build_root: PathBuf,
    pub expected_identity: ProjectEditorCompositionIdentity,
    #[serde(default)]
    pub cache_policy: ProjectEditorCompositionCachePolicy,
    #[serde(default)]
    pub qos_policy: ProjectEditorCompositionBuildQosPolicy,
    pub deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_executable: Option<PathBuf>,
    pub cargo_identity: String,
    #[serde(default = "default_composition_capture_limit_bytes")]
    pub capture_limit_bytes: usize,
}

impl ProjectEditorCompositionBuildRequest {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.build_request_schema_unsupported",
                format!(
                    "Unsupported composition build request schema: {}",
                    self.schema_version
                ),
            ));
        }
        if self.project_root.as_os_str().is_empty()
            || self.engine_sdk_root.as_os_str().is_empty()
            || self.build_root.as_os_str().is_empty()
            || self.cargo_identity.trim().is_empty()
            || self.capture_limit_bytes == 0
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.build_request_field_required",
                "Composition build paths, timeout, and capture limit are required.",
            ));
        }
        self.cache_policy.validate()?;
        self.qos_policy.validate()?;
        self.deadline_policy.validate()?;
        self.expected_identity.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionQualificationKind {
    Headless,
    RealWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionQualificationSeal {
    pub schema_version: String,
    pub qualification_kind: ProjectEditorCompositionQualificationKind,
    pub qualification_report_schema_version: String,
    pub qualification_report_file_name: String,
    pub qualification_report_digest: String,
    pub composition_identity_digest: String,
    pub resolved_identity: ProjectEditorCompositionResolvedIdentity,
    pub executable_hash: String,
    pub descriptor_hash: String,
    pub build_report_hash: String,
    pub sealed_at: u64,
}

impl ProjectEditorCompositionQualificationSeal {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.promotion_qualification_mismatch",
                "Unsupported composition qualification seal schema.",
            ));
        }
        if self.qualification_report_schema_version.trim().is_empty()
            || !is_portable_file_name(&self.qualification_report_file_name)
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.promotion_qualification_mismatch",
                "Qualification seal must name one supported report and a portable sibling file.",
            ));
        }
        for digest in [
            &self.qualification_report_digest,
            &self.composition_identity_digest,
            &self.resolved_identity.requested_identity_digest,
            &self.executable_hash,
            &self.descriptor_hash,
            &self.build_report_hash,
        ] {
            if !is_sha256_identity(digest) {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.promotion_qualification_mismatch",
                    "Qualification seal digests must use canonical SHA-256 values.",
                ));
            }
        }
        self.resolved_identity.validate()?;
        if self.composition_identity_digest != self.resolved_identity.requested_identity_digest {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.resolved_identity_mismatch",
                "Qualification seal requested and resolved identities must agree.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionPromotionRequest {
    pub schema_version: String,
    pub authority_operation_id: String,
    pub authorized_run_root: PathBuf,
    pub source_artifact_root: PathBuf,
    pub destination_cache_root: PathBuf,
    pub backup_root: PathBuf,
    pub qualification_seal_path: PathBuf,
    pub expected_identity: ProjectEditorCompositionIdentity,
    pub expected_resolved_identity: ProjectEditorCompositionResolvedIdentity,
}

impl ProjectEditorCompositionPromotionRequest {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.schema_version != PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.promotion_schema_unsupported",
                "Unsupported project Editor composition promotion request schema.",
            ));
        }
        if !is_portable_operation_id(&self.authority_operation_id) {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.promotion_authority_missing",
                "Promotion requires one portable authority operation id.",
            ));
        }
        for path in [
            &self.authorized_run_root,
            &self.source_artifact_root,
            &self.destination_cache_root,
            &self.backup_root,
            &self.qualification_seal_path,
        ] {
            if path.as_os_str().is_empty() || !path.is_absolute() {
                return Err(ProjectEditorCompositionContractError::new(
                    "project_editor_composition.promotion_path_invalid",
                    "Promotion paths must be explicit absolute paths.",
                ));
            }
        }
        if self.source_artifact_root == self.destination_cache_root
            || self.source_artifact_root == self.backup_root
            || self.destination_cache_root == self.backup_root
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.promotion_path_invalid",
                "Promotion source, destination, and backup roots must be distinct.",
            ));
        }
        self.expected_identity.validate()?;
        self.expected_resolved_identity.validate()?;
        let requested = self.expected_identity.digest().map_err(|error| {
            ProjectEditorCompositionContractError::new(
                "project_editor_composition.resolved_identity_mismatch",
                error.to_string(),
            )
        })?;
        if requested != self.expected_resolved_identity.requested_identity_digest {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.resolved_identity_mismatch",
                "Promotion requested and resolved identities must agree.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionPromotionStatus {
    ExactCacheHit,
    Promoted,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionPromotionStage {
    ValidateRequest,
    ValidateDestination,
    ValidateSource,
    Copy,
    Publish,
    Verify,
    Rollback,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionPromotionBackupStatus {
    NotRequired,
    Created,
    Retained,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionPromotionRollbackStatus {
    NotRequired,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionPromotionCleanupStatus {
    Complete,
    Retained,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionPromotionReport {
    pub schema_version: String,
    pub status: ProjectEditorCompositionPromotionStatus,
    pub stage: ProjectEditorCompositionPromotionStage,
    pub authority_operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_source_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_destination_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_identity_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_identity_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_resolved_identity: Option<ProjectEditorCompositionResolvedIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_resolved_identity: Option<ProjectEditorCompositionResolvedIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_executable_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_executable_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_executable_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_report_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification_evidence_digest: Option<String>,
    pub backup_status: ProjectEditorCompositionPromotionBackupStatus,
    pub rollback_status: ProjectEditorCompositionPromotionRollbackStatus,
    pub cleanup_status: ProjectEditorCompositionPromotionCleanupStatus,
    pub retained_paths: Vec<PathBuf>,
    pub diagnostics: Vec<ProjectEditorCompositionDiagnostic>,
}

fn is_portable_operation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_portable_file_name(value: &str) -> bool {
    is_portable_operation_id(value) && value != "." && value != ".."
}

impl ProjectEditorCompositionCachePolicy {
    pub fn validate(&self) -> Result<(), ProjectEditorCompositionContractError> {
        if self.global_soft_limit_bytes == 0
            || self.per_project_soft_limit_bytes == 0
            || self.global_soft_limit_bytes > self.global_hard_limit_bytes
            || self.per_project_soft_limit_bytes > self.per_project_hard_limit_bytes
            || self.per_project_hard_limit_bytes > self.global_hard_limit_bytes
        {
            return Err(ProjectEditorCompositionContractError::new(
                "project_editor_composition.cache_policy_invalid",
                "Composition cache limits must be non-zero and ordered soft <= hard <= global hard.",
            ));
        }
        Ok(())
    }
}

fn default_composition_capture_limit_bytes() -> usize {
    DEFAULT_COMPOSITION_CAPTURE_LIMIT_BYTES
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEditorCompositionContractError {
    pub code: String,
    pub message: String,
}

impl ProjectEditorCompositionContractError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ProjectEditorCompositionContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectEditorCompositionContractError {}

fn is_sha256_identity(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionDescriptor {
    pub schema_version: String,
    pub identity: ProjectEditorCompositionIdentity,
    pub identity_digest: String,
    pub resolved_identity: ProjectEditorCompositionResolvedIdentity,
    pub executable_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionArtifact {
    pub schema_version: String,
    pub executable_path: PathBuf,
    pub descriptor_path: PathBuf,
    pub build_report_path: PathBuf,
    pub descriptor: ProjectEditorCompositionDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionBuildStatus {
    Success,
    Failed,
    TrustRequired,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionBuildSourceKind {
    ExactCache,
    ExactPromotion,
    ControlledBuild,
    #[default]
    NotDetermined,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionCacheStatus {
    #[default]
    NotChecked,
    Hit,
    Promoted,
    Miss,
    Invalidated,
    Rebuilt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionProcessPriority {
    #[default]
    Normal,
    BelowNormal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionDiagnostic {
    pub code: String,
    pub stage: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_identity: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildReport {
    pub schema_version: String,
    pub status: ProjectEditorCompositionBuildStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<ProjectEditorCompositionIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_identity: Option<ProjectEditorCompositionResolvedIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ProjectEditorCompositionArtifact>,
    #[serde(default)]
    pub source_kind: ProjectEditorCompositionBuildSourceKind,
    pub cache_status: ProjectEditorCompositionCacheStatus,
    pub cleanup_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_size_bytes: Option<u64>,
    pub steps: Vec<ProjectEditorCompositionBuildStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_policy: Option<ProjectEditorCompositionBuildDeadlinePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_policy: Option<ProjectEditorCompositionBuildQosPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_facts: Option<ProjectEditorCompositionSystemFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_decision: Option<ProjectEditorCompositionBuildQosDecision>,
    #[serde(default)]
    pub requested_priority: ProjectEditorCompositionProcessPriority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_priority: Option<ProjectEditorCompositionProcessPriority>,
    #[serde(default)]
    pub priority_applied: bool,
    #[serde(default)]
    pub cancellation_requested: bool,
    #[serde(default)]
    pub process_tree_terminated: bool,
    #[serde(default)]
    pub output_readers_joined: bool,
    #[serde(default)]
    pub root_wait_completed: bool,
    #[serde(default)]
    pub process_group_released: bool,
    #[serde(default)]
    pub owned_process_cleanup_confirmed: bool,
    #[serde(default)]
    pub release_build_soft_budget_exceeded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_build_soft_budget_exceeded_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compilation_cache_compatibility_digest: Option<String>,
    #[serde(default)]
    pub compilation_cache_affinity: ProjectEditorCompositionCompilationCacheAffinity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_target_anchor_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_target_root_digest: Option<String>,
    #[serde(default)]
    pub cross_root_portable: bool,
    #[serde(default)]
    pub worker_joined: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redraw_policy_hz: Option<u16>,
    #[serde(default)]
    pub stage_durations_ms: BTreeMap<String, u128>,
    pub diagnostics: Vec<ProjectEditorCompositionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionBuildStep {
    pub stage: String,
    pub command: Vec<String>,
    pub timeout_ms: u64,
    pub process: BoundedChildProcessResult,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionCompilationCacheAffinity {
    SameRootHit,
    PathAffineMiss,
    #[default]
    Cold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionHandoffTicket {
    pub schema_version: String,
    pub nonce: String,
    pub old_editor_instance_id: String,
    pub expected_identity: ProjectEditorCompositionIdentity,
    pub expected_identity_digest: String,
    pub project_root: PathBuf,
    pub project_id: String,
    pub artifact_executable_path: PathBuf,
    pub artifact_executable_hash: String,
    pub workspace_state_ref: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub acknowledgement_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEditorCompositionLaunchStatus {
    Pending,
    Ready,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorCompositionLaunchReceipt {
    pub schema_version: String,
    pub status: ProjectEditorCompositionLaunchStatus,
    pub nonce: String,
    pub old_editor_instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_editor_instance_id: Option<String>,
    pub project_id: String,
    pub composition_identity_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_process_id: Option<u32>,
    pub diagnostics: Vec<ProjectEditorCompositionDiagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ProjectEditorCompositionIdentity {
        ProjectEditorCompositionIdentity {
            schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: "fixture.project".to_string(),
            module_id: "fixture.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc 1.96.0".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
            normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
            dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
        }
    }

    fn resolved_identity() -> ProjectEditorCompositionResolvedIdentity {
        ProjectEditorCompositionResolvedIdentity::new(
            identity().digest().unwrap(),
            &GeneratedCompositionLockLineage {
                schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
                lock_input_digest: format!("sha256:{}", "1".repeat(64)),
                raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
                resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
            },
        )
        .unwrap()
    }

    #[test]
    fn project_editor_composition_contract_schemas_round_trip_and_reject_unknown_fields() {
        let request = ProjectEditorCompositionBuildRequest {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: PathBuf::from("G:/fixture/project"),
            engine_sdk_root: PathBuf::from("G:/fixture/sdk"),
            build_root: PathBuf::from("G:/fixture/build"),
            expected_identity: identity(),
            cache_policy: ProjectEditorCompositionCachePolicy::default(),
            qos_policy: ProjectEditorCompositionBuildQosPolicy::default(),
            deadline_policy: ProjectEditorCompositionBuildDeadlinePolicy::default(),
            cargo_executable: None,
            cargo_identity: "cargo 1.96.0".to_string(),
            capture_limit_bytes: DEFAULT_COMPOSITION_CAPTURE_LIMIT_BYTES,
        };
        request.validate().unwrap();
        let mut request_value = serde_json::to_value(&request).unwrap();
        assert_eq!(
            serde_json::from_value::<ProjectEditorCompositionBuildRequest>(request_value.clone())
                .unwrap(),
            request
        );
        request_value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(
            serde_json::from_value::<ProjectEditorCompositionBuildRequest>(request_value).is_err()
        );

        let descriptor = ProjectEditorCompositionDescriptor {
            schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity(),
            identity_digest: identity().digest().unwrap(),
            resolved_identity: resolved_identity(),
            executable_hash: "sha256:executable".to_string(),
            created_at: 7,
        };
        let artifact = ProjectEditorCompositionArtifact {
            schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
            executable_path: PathBuf::from("editor.exe"),
            descriptor_path: PathBuf::from("composition-descriptor.json"),
            build_report_path: PathBuf::from("build-report.json"),
            descriptor,
        };
        let report = ProjectEditorCompositionBuildReport {
            schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionBuildStatus::Success,
            identity: Some(identity()),
            identity_digest: Some(identity().digest().unwrap()),
            resolved_identity: Some(resolved_identity()),
            artifact: Some(artifact),
            source_kind: ProjectEditorCompositionBuildSourceKind::ControlledBuild,
            cache_status: ProjectEditorCompositionCacheStatus::Rebuilt,
            cleanup_status: "staging_published".to_string(),
            artifact_size_bytes: Some(42),
            steps: Vec::new(),
            deadline_policy: Some(ProjectEditorCompositionBuildDeadlinePolicy::default()),
            qos_policy: Some(ProjectEditorCompositionBuildQosPolicy::default()),
            system_facts: Some(ProjectEditorCompositionSystemFacts {
                logical_processors: Some(12),
                available_memory_bytes: Some(7 * GIBIBYTE),
            }),
            qos_decision: Some(
                resolve_project_editor_composition_build_qos(
                    &ProjectEditorCompositionBuildQosPolicy::default(),
                    ProjectEditorCompositionSystemFacts {
                        logical_processors: Some(12),
                        available_memory_bytes: Some(7 * GIBIBYTE),
                    },
                )
                .unwrap(),
            ),
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
        assert_eq!(
            serde_json::from_str::<ProjectEditorCompositionBuildReport>(
                &serde_json::to_string(&report).unwrap()
            )
            .unwrap(),
            report
        );
        let mut legacy_report_value = serde_json::to_value(&report).unwrap();
        let legacy_report = legacy_report_value.as_object_mut().unwrap();
        legacy_report.insert(
            "schemaVersion".to_string(),
            serde_json::json!(PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1),
        );
        for field in [
            "sourceKind",
            "qosPolicy",
            "systemFacts",
            "qosDecision",
            "requestedPriority",
            "effectivePriority",
            "priorityApplied",
            "cancellationRequested",
            "processTreeTerminated",
            "workerJoined",
            "redrawPolicyHz",
            "stageDurationsMs",
        ] {
            legacy_report.remove(field);
        }
        let parsed_legacy: ProjectEditorCompositionBuildReport =
            serde_json::from_value(legacy_report_value).unwrap();
        assert_eq!(
            parsed_legacy.schema_version,
            PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1
        );
        assert_eq!(
            parsed_legacy.source_kind,
            ProjectEditorCompositionBuildSourceKind::NotDetermined
        );

        let ticket = ProjectEditorCompositionHandoffTicket {
            schema_version: PROJECT_EDITOR_COMPOSITION_HANDOFF_TICKET_SCHEMA_VERSION.to_string(),
            nonce: "nonce".to_string(),
            old_editor_instance_id: "old".to_string(),
            expected_identity: identity(),
            expected_identity_digest: identity().digest().unwrap(),
            project_root: PathBuf::from("G:/fixture/project"),
            project_id: "fixture.project".to_string(),
            artifact_executable_path: PathBuf::from("editor.exe"),
            artifact_executable_hash: "sha256:executable".to_string(),
            workspace_state_ref: "workspace-state".to_string(),
            created_at: 7,
            expires_at: 8,
            acknowledgement_path: PathBuf::from("ack.json"),
        };
        assert_eq!(
            serde_json::from_str::<ProjectEditorCompositionHandoffTicket>(
                &serde_json::to_string(&ticket).unwrap()
            )
            .unwrap(),
            ticket
        );
        let receipt = ProjectEditorCompositionLaunchReceipt {
            schema_version: PROJECT_EDITOR_COMPOSITION_LAUNCH_RECEIPT_SCHEMA_VERSION.to_string(),
            status: ProjectEditorCompositionLaunchStatus::Ready,
            nonce: "nonce".to_string(),
            old_editor_instance_id: "old".to_string(),
            new_editor_instance_id: Some("new".to_string()),
            project_id: "fixture.project".to_string(),
            composition_identity_digest: identity().digest().unwrap(),
            candidate_process_id: Some(42),
            diagnostics: Vec::new(),
        };
        assert_eq!(
            serde_json::from_str::<ProjectEditorCompositionLaunchReceipt>(
                &serde_json::to_string(&receipt).unwrap()
            )
            .unwrap(),
            receipt
        );

        let mut wrong_schema = request.clone();
        wrong_schema.schema_version = "project-editor-composition-build-request.v4".to_string();
        assert_eq!(
            wrong_schema.validate().unwrap_err().code,
            "project_editor_composition.build_request_schema_unsupported"
        );

        let mut missing_identity = request.clone();
        missing_identity.expected_identity.module_id = "  ".to_string();
        assert_eq!(
            missing_identity.validate().unwrap_err().code,
            "project_editor_composition.identity_field_required"
        );

        let mut malformed_digest = request;
        malformed_digest.expected_identity.engine_sdk_digest = "sha256:not-a-digest".to_string();
        assert_eq!(
            malformed_digest.validate().unwrap_err().code,
            "project_editor_composition.identity_digest_invalid"
        );
    }

    #[test]
    fn project_editor_composition_contract_identity_tracks_every_build_domain() {
        let baseline = identity();
        let baseline_digest = baseline.digest().unwrap();
        let mutations: Vec<Box<dyn Fn(&mut ProjectEditorCompositionIdentity)>> = vec![
            Box::new(|value| value.project_id.push_str(".changed")),
            Box::new(|value| value.module_id.push_str(".changed")),
            Box::new(|value| value.interface_version.push_str(".changed")),
            Box::new(|value| value.aot_content_digest.push_str(".changed")),
            Box::new(|value| value.editor_build_identity.push_str(".changed")),
            Box::new(|value| value.engine_sdk_digest.push_str(".changed")),
            Box::new(|value| value.toolchain_identity.push_str(".changed")),
            Box::new(|value| value.target_triple.push_str(".changed")),
            Box::new(|value| value.profile.push_str(".changed")),
            Box::new(|value| value.normalized_manifest_digest.push_str(".changed")),
            Box::new(|value| value.normalized_dependency_digest.push_str(".changed")),
            Box::new(|value| value.dependency_lock_digest.push_str(".changed")),
        ];
        for mutate in mutations {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(changed.digest().unwrap(), baseline_digest);
        }
    }

    #[test]
    fn project_editor_composition_qos_resolves_cpu_memory_and_policy_caps() {
        let policy = ProjectEditorCompositionBuildQosPolicy::default();
        let decision = resolve_project_editor_composition_build_qos(
            &policy,
            ProjectEditorCompositionSystemFacts {
                logical_processors: Some(12),
                available_memory_bytes: Some(7 * 1024 * 1024 * 1024),
            },
        )
        .unwrap();

        assert_eq!(decision.resolved_jobs, 4);
        assert_eq!(decision.cpu_limited_jobs, Some(8));
        assert_eq!(decision.memory_limited_jobs, Some(4));
        assert!(!decision.used_conservative_fallback);
    }

    #[test]
    fn project_editor_composition_qos_uses_conservative_nonzero_fallback() {
        let policy = ProjectEditorCompositionBuildQosPolicy::default();
        let decision = resolve_project_editor_composition_build_qos(
            &policy,
            ProjectEditorCompositionSystemFacts {
                logical_processors: None,
                available_memory_bytes: None,
            },
        )
        .unwrap();

        assert_eq!(decision.resolved_jobs, policy.min_jobs);
        assert!(decision.used_conservative_fallback);
        assert!(decision
            .diagnostics
            .iter()
            .any(|code| { code == "project_editor_composition.qos_system_facts_unavailable" }));
    }

    #[test]
    fn project_editor_composition_qos_rejects_zero_and_inverted_policy() {
        let mut policy = ProjectEditorCompositionBuildQosPolicy::default();
        policy.max_jobs = 0;
        assert_eq!(
            resolve_project_editor_composition_build_qos(
                &policy,
                ProjectEditorCompositionSystemFacts::default(),
            )
            .unwrap_err()
            .code,
            "project_editor_composition.qos_policy_invalid"
        );

        let mut policy = ProjectEditorCompositionBuildQosPolicy::default();
        policy.min_jobs = policy.max_jobs + 1;
        assert!(resolve_project_editor_composition_build_qos(
            &policy,
            ProjectEditorCompositionSystemFacts::default(),
        )
        .is_err());
    }

    #[cfg(windows)]
    #[test]
    fn project_editor_composition_qos_collects_windows_memory_facts() {
        let facts = ProjectEditorCompositionSystemFacts::collect();

        assert!(facts.logical_processors.is_some_and(|value| value > 0));
        assert!(facts.available_memory_bytes.is_some_and(|value| value > 0));
    }

    #[test]
    fn project_editor_composition_report_v2_reads_v1_and_rejects_unknown_fields() {
        let legacy = serde_json::json!({
            "schemaVersion": PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION_V1,
            "status": "failed",
            "cacheStatus": "not_checked",
            "cleanupStatus": "not_required",
            "steps": [],
            "diagnostics": []
        });
        let parsed: ProjectEditorCompositionBuildReport = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            parsed.source_kind,
            ProjectEditorCompositionBuildSourceKind::NotDetermined
        );
        assert_eq!(
            parsed.requested_priority,
            ProjectEditorCompositionProcessPriority::Normal
        );

        let mut current = parsed;
        current.schema_version = PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION.to_string();
        current.source_kind = ProjectEditorCompositionBuildSourceKind::ControlledBuild;
        current.qos_policy = Some(ProjectEditorCompositionBuildQosPolicy::default());
        let encoded = serde_json::to_value(&current).unwrap();
        assert_eq!(
            serde_json::from_value::<ProjectEditorCompositionBuildReport>(encoded.clone()).unwrap(),
            current
        );
        let mut unknown = encoded;
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(serde_json::from_value::<ProjectEditorCompositionBuildReport>(unknown).is_err());
    }

    #[test]
    fn project_editor_composition_promotion_schema_red_contract() {
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_PROMOTION_REQUEST_SCHEMA_VERSION,
            "project-editor-composition-promotion-request.v2"
        );
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_PROMOTION_REPORT_SCHEMA_VERSION,
            "project-editor-composition-promotion-report.v2"
        );
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_QUALIFICATION_SEAL_SCHEMA_VERSION,
            "project-editor-composition-qualification-seal.v2"
        );
    }

    fn fixture_lock(root_name: &str, checksum: &str, dependency: &str) -> Vec<u8> {
        format!(
            r#"version = 3

[[package]]
name = "{root_name}"
version = "0.0.2"
dependencies = ["serde"]

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{checksum}"
dependencies = ["{dependency}"]

[[package]]
name = "serde_core"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{}"
"#,
            "b".repeat(64)
        )
        .into_bytes()
    }

    #[test]
    fn project_editor_composition_r1_contract_versions_and_deadlines_are_strict() {
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
            "project-editor-composition-build-request.v3"
        );
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_BUILD_REPORT_SCHEMA_VERSION,
            "project-editor-composition-build-report.v3"
        );
        assert_eq!(
            PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
            "project-editor-composition-artifact.v2"
        );
        let policy = ProjectEditorCompositionBuildDeadlinePolicy::default();
        policy.validate().unwrap();
        let mut invalid = policy;
        invalid.release_build_soft_budget_ms = invalid.release_build_hard_deadline_ms;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "project_editor_composition.deadline_policy_invalid"
        );
    }

    #[test]
    fn project_editor_composition_lock_lineage_separates_raw_bytes_from_resolved_graph() {
        let input = format!("sha256:{}", "a".repeat(64));
        let left = fixture_lock("generated-left", &"c".repeat(64), "serde_core");
        let right = fixture_lock("generated-right", &"c".repeat(64), "serde_core");
        let left_lineage =
            generated_composition_lock_lineage(&left, "generated-left", input.clone()).unwrap();
        let right_lineage =
            generated_composition_lock_lineage(&right, "generated-right", input).unwrap();
        assert_ne!(left_lineage.raw_lock_digest, right_lineage.raw_lock_digest);
        assert_eq!(
            left_lineage.resolved_graph_digest,
            right_lineage.resolved_graph_digest
        );
    }

    #[test]
    fn project_editor_composition_lock_lineage_tracks_graph_edges_and_checksums() {
        let input = format!("sha256:{}", "a".repeat(64));
        let baseline = generated_composition_lock_lineage(
            &fixture_lock("generated", &"c".repeat(64), "serde_core"),
            "generated",
            input.clone(),
        )
        .unwrap();
        let changed_checksum = generated_composition_lock_lineage(
            &fixture_lock("generated", &"d".repeat(64), "serde_core"),
            "generated",
            input.clone(),
        )
        .unwrap();
        let changed_edge = generated_composition_lock_lineage(
            &fixture_lock("generated", &"c".repeat(64), "serde_core 1.0.0"),
            "generated",
            input,
        )
        .unwrap();
        assert_ne!(
            baseline.resolved_graph_digest,
            changed_checksum.resolved_graph_digest
        );
        assert_ne!(
            baseline.resolved_graph_digest,
            changed_edge.resolved_graph_digest
        );
    }

    #[test]
    fn project_editor_composition_resolved_identity_tracks_lineage() {
        let requested = identity().digest().unwrap();
        let mut lineage = generated_composition_lock_lineage(
            &fixture_lock("generated", &"c".repeat(64), "serde_core"),
            "generated",
            format!("sha256:{}", "a".repeat(64)),
        )
        .unwrap();
        let baseline =
            ProjectEditorCompositionResolvedIdentity::new(requested.clone(), &lineage).unwrap();
        lineage.raw_lock_digest = format!("sha256:{}", "e".repeat(64));
        let changed = ProjectEditorCompositionResolvedIdentity::new(requested, &lineage).unwrap();
        assert_ne!(baseline.lock_lineage_digest, changed.lock_lineage_digest);
        assert_ne!(
            baseline.resolved_artifact_key_digest,
            changed.resolved_artifact_key_digest
        );
    }
}
