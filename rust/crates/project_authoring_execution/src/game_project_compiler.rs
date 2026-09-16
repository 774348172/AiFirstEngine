use authoring_project_context::{
    ProjectQualification, ProjectSnapshot, ProjectSnapshotLease, SnapshotRetentionPolicy,
    PROJECT_SNAPSHOT_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::RuntimePackageBuildInput;
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationReport,
    ExportedPlayerProcessVerificationRequest, ExportedPlayerProcessVerificationStatus,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

mod check;
pub use check::{CheckDiagnostic, SourceLocation};
mod playtest;
pub use playtest::{PreparedPlaytestScenario, ProjectPlaytestReport};

const PREPARED_RUNTIME_PACKAGE_IDENTITY_VERSION: &str = "prepared-runtime-package-identity.v1";
const PROJECT_MANIFEST_PATH: &str = "project.aife.json";
const DEFAULT_WINDOWS_DEV_PROFILE_PATH: &str = "BuildProfiles/windows.dev.json";
const DEFAULT_WINDOWS_RELEASE_PROFILE_PATH: &str = "BuildProfiles/windows.release.json";
const DEFAULT_ANDROID_DEV_PROFILE_PATH: &str = "BuildProfiles/android.dev.json";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerProjectManifest {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    engine_version: String,
    #[serde(default)]
    default_scene: Option<String>,
    #[serde(default)]
    asset_root: Option<String>,
    #[serde(default)]
    settings_version: Option<String>,
    #[serde(default)]
    runtime_module: Option<CompilerRuntimeModuleSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompilerRuntimeModuleSpec {
    #[serde(default)]
    pub(crate) module_id: String,
    #[serde(default)]
    pub(crate) interface_version: String,
    #[serde(default)]
    pub(crate) cargo_manifest: String,
    #[serde(default)]
    pub(crate) cargo_package: String,
    #[serde(default)]
    pub(crate) player_binary: String,
    #[serde(default)]
    pub(crate) project_game_sdk: String,
}

/// Immutable compiler input reconstructed only from an operation-owned snapshot.
/// The assembler consumes this view instead of reopening the live project tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompilerSourceView {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

impl CompilerSourceView {
    fn from_snapshot(snapshot: &ProjectSnapshot) -> Result<Self, GameProjectCompilerError> {
        validate_lease_owned_sources(snapshot)?;
        let files = snapshot
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.bytes.clone()))
            .collect();
        Ok(Self { files })
    }

    pub(crate) fn len(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn bytes(&self, relative_path: &str) -> Option<&[u8]> {
        self.files.get(relative_path).map(Vec::as_slice)
    }

    pub(crate) fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub(crate) fn project_manifest_value(
        &self,
    ) -> Result<serde_json::Value, GameProjectCompilerError> {
        let bytes = self.bytes(PROJECT_MANIFEST_PATH).ok_or_else(|| {
            compiler_error(
                "game_project_compiler.manifest_missing_from_snapshot",
                "The snapshot lease does not own project.aife.json.",
                GameProjectCompilerStage::Prepare,
                "Acquire a compiler snapshot lease that includes the canonical project manifest.",
            )
        })?;
        serde_json::from_slice(bytes).map_err(|error| {
            compiler_error(
                "game_project_compiler.manifest_invalid",
                format!("The leased project manifest is not valid JSON: {error}"),
                GameProjectCompilerStage::Prepare,
                "Repair project.aife.json, refresh the project, and acquire a new snapshot lease.",
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetProfile {
    WindowsDev,
    WindowsRelease,
    AndroidDev,
}

impl TargetProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDev => "windows-dev",
            Self::WindowsRelease => "windows-release",
            Self::AndroidDev => "android-dev",
        }
    }

    fn source_path(self) -> &'static str {
        match self {
            Self::WindowsDev => DEFAULT_WINDOWS_DEV_PROFILE_PATH,
            Self::WindowsRelease => DEFAULT_WINDOWS_RELEASE_PROFILE_PATH,
            Self::AndroidDev => DEFAULT_ANDROID_DEV_PROFILE_PATH,
        }
    }

    fn expected_values(self) -> (&'static str, &'static str) {
        match self {
            Self::WindowsDev => ("windows", "dev"),
            Self::WindowsRelease => ("windows", "release"),
            Self::AndroidDev => ("android", "dev"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Headless,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    mode: ExecutionMode,
    frame_limit: u64,
    timeout_ms: u64,
    project_root: Option<PathBuf>,
    project_relative_output: Option<crate::ProjectRelativePath>,
    player_artifact_build_root: Option<PathBuf>,
    verify_player: bool,
}

impl RunOptions {
    pub fn new(mode: ExecutionMode, frame_limit: u64, timeout_ms: u64) -> Self {
        Self {
            mode,
            frame_limit,
            timeout_ms,
            project_root: None,
            project_relative_output: None,
            player_artifact_build_root: None,
            verify_player: true,
        }
    }

    pub fn with_project_delivery(
        mut self,
        project_root: impl Into<PathBuf>,
        project_relative_output: crate::ProjectRelativePath,
        player_artifact_build_root: impl Into<PathBuf>,
    ) -> Self {
        self.project_root = Some(project_root.into());
        self.project_relative_output = Some(project_relative_output);
        self.player_artifact_build_root = Some(player_artifact_build_root.into());
        self
    }

    pub fn with_player_verification(mut self, enabled: bool) -> Self {
        self.verify_player = enabled;
        self
    }

    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    pub fn frame_limit(&self) -> u64 {
        self.frame_limit
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    target_profile: TargetProfile,
    project_root: Option<PathBuf>,
    project_relative_output: Option<crate::ProjectRelativePath>,
    player_artifact_build_root: Option<PathBuf>,
    frame_limit: u64,
    timeout_ms: u64,
    verify_player: bool,
}

impl BuildRequest {
    pub fn new(target_profile: TargetProfile) -> Self {
        Self {
            target_profile,
            project_root: None,
            project_relative_output: None,
            player_artifact_build_root: None,
            frame_limit: 3,
            timeout_ms: 30_000,
            verify_player: true,
        }
    }

    pub fn for_project(
        target_profile: TargetProfile,
        project_root: impl Into<PathBuf>,
        project_relative_output: crate::ProjectRelativePath,
        player_artifact_build_root: impl Into<PathBuf>,
    ) -> Self {
        Self::new(target_profile).with_project_delivery(
            project_root,
            project_relative_output,
            player_artifact_build_root,
        )
    }

    pub fn with_project_delivery(
        mut self,
        project_root: impl Into<PathBuf>,
        project_relative_output: crate::ProjectRelativePath,
        player_artifact_build_root: impl Into<PathBuf>,
    ) -> Self {
        self.project_root = Some(project_root.into());
        self.project_relative_output = Some(project_relative_output);
        self.player_artifact_build_root = Some(player_artifact_build_root.into());
        self
    }

    pub fn with_player_verification(mut self, enabled: bool) -> Self {
        self.verify_player = enabled;
        self
    }

    pub fn with_frame_limit(mut self, frame_limit: u64) -> Self {
        self.frame_limit = frame_limit;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms.max(1);
        self
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyRequest {
    mode: ExecutionMode,
    frame_limit: u64,
    timeout_ms: u64,
    screenshot: bool,
}

impl VerifyRequest {
    pub fn new(mode: ExecutionMode, frame_limit: u64, timeout_ms: u64, screenshot: bool) -> Self {
        Self {
            mode,
            frame_limit,
            timeout_ms,
            screenshot,
        }
    }

    pub fn mode(self) -> ExecutionMode {
        self.mode
    }

    pub fn frame_limit(self) -> u64 {
        self.frame_limit
    }

    pub fn timeout_ms(self) -> u64 {
        self.timeout_ms
    }

    pub fn screenshot(self) -> bool {
        self.screenshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectArtifactLineage {
    project_identity: String,
    opened_project_binding: String,
    revision_id: String,
    snapshot_id: String,
    target_profile: TargetProfile,
    source_identity: String,
}

impl ProjectArtifactLineage {
    pub fn project_identity(&self) -> &str {
        &self.project_identity
    }

    pub fn opened_project_binding(&self) -> &str {
        &self.opened_project_binding
    }

    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub fn source_identity(&self) -> &str {
        &self.source_identity
    }
}

#[derive(Debug, Clone)]
pub struct PreparedRuntimePackage {
    source: CompilerSourceView,
    lineage: ProjectArtifactLineage,
    preparation_identity: String,
    source_file_count: usize,
    runtime_package_identity: String,
    runtime_package_build_input: RuntimePackageBuildInput,
    assembly_digest: String,
    producer_reports: Vec<crate::ProjectAssemblyProducerReport>,
    generated_runtime_glue: Option<crate::PreparedRuntimeGlue>,
}

impl PreparedRuntimePackage {
    pub(crate) fn source(&self) -> &CompilerSourceView {
        &self.source
    }

    pub fn prepare_summary(&self) -> serde_json::Value {
        use crate::ProjectAssemblyArtifactCacheStatus as Status;
        serde_json::json!({
            "preparationIdentity": self.preparation_identity,
            "sourceIdentity": self.lineage.source_identity,
            "targetProfile": self.lineage.target_profile,
            "producerCount": self.producer_reports.len(),
            "reusedCount": self.producer_reports.iter().filter(|r| r.cache_status == Status::Hit).count(),
            "producedCount": self.producer_reports.iter().filter(|r| r.cache_status != Status::Hit).count(),
            "missReasons": self.producer_reports.iter().filter_map(|r| r.miss_reason.as_deref()).collect::<std::collections::BTreeSet<_>>(),
            "uncachedStages": ["source-validation", "scene-prefab-rule-aui-input", "runtime-glue", "package-assembly"]
        })
    }
    pub fn lineage(&self) -> &ProjectArtifactLineage {
        &self.lineage
    }

    pub fn preparation_identity(&self) -> &str {
        &self.preparation_identity
    }

    pub fn source_file_count(&self) -> usize {
        self.source_file_count
    }

    pub fn runtime_package_identity(&self) -> Option<&str> {
        Some(&self.runtime_package_identity)
    }

    pub fn is_runtime_package_ready(&self) -> bool {
        !self.runtime_package_identity.is_empty()
    }

    pub fn runtime_package_build_input(&self) -> &RuntimePackageBuildInput {
        &self.runtime_package_build_input
    }

    pub fn assembly_digest(&self) -> &str {
        &self.assembly_digest
    }

    pub fn producer_reports(&self) -> &[crate::ProjectAssemblyProducerReport] {
        &self.producer_reports
    }

    pub fn generated_runtime_glue_report(&self) -> Option<&crate::GeneratedRuntimeGlueReport> {
        self.generated_runtime_glue
            .as_ref()
            .map(crate::PreparedRuntimeGlue::report)
    }

    pub fn generated_runtime_glue(&self) -> Option<&crate::PreparedRuntimeGlue> {
        self.generated_runtime_glue.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeExecutionReport {
    lineage: ProjectArtifactLineage,
    preparation_identity: String,
    delivery: DeliveryRef,
    desktop_export: crate::DesktopExportReport,
}

impl RuntimeExecutionReport {
    pub fn lineage(&self) -> &ProjectArtifactLineage {
        &self.lineage
    }

    pub fn preparation_identity(&self) -> &str {
        &self.preparation_identity
    }

    pub fn delivery(&self) -> &DeliveryRef {
        &self.delivery
    }

    pub fn desktop_export(&self) -> &crate::DesktopExportReport {
        &self.desktop_export
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRef {
    lineage: ProjectArtifactLineage,
    preparation_identity: String,
    delivery_identity: String,
    package_dir: PathBuf,
    artifact_identity: String,
    runtime_package_digest: String,
    manifest_digest: String,
    evidence_refs: Vec<String>,
}

impl DeliveryRef {
    pub fn lineage(&self) -> &ProjectArtifactLineage {
        &self.lineage
    }

    pub fn preparation_identity(&self) -> &str {
        &self.preparation_identity
    }

    pub fn delivery_identity(&self) -> &str {
        &self.delivery_identity
    }

    pub fn package_dir(&self) -> &Path {
        &self.package_dir
    }

    pub fn artifact_identity(&self) -> &str {
        &self.artifact_identity
    }

    pub fn evidence_refs(&self) -> &[String] {
        &self.evidence_refs
    }

    fn ensure_manifest_identity(&self) -> Result<(), GameProjectCompilerError> {
        let path = self.package_dir.join("package-manifest.json");
        let actual = runtime_cli::semantic_file_digest(&path).map_err(|message| {
            compiler_error(
                "game_project_compiler.delivery_changed",
                format!(
                    "Cannot read retained delivery manifest {}: {message}",
                    path.display()
                ),
                GameProjectCompilerStage::Verify,
                "Use the original delivery or explicitly build a new one.",
            )
        })?;
        if actual != self.manifest_digest {
            return Err(compiler_error(
                "game_project_compiler.delivery_changed",
                format!(
                    "Delivery manifest {} no longer matches the retained delivery identity.",
                    path.display()
                ),
                GameProjectCompilerStage::Verify,
                "Use the original delivery or explicitly build a new one.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildDeliveryReport {
    delivery: DeliveryRef,
    desktop_export: crate::DesktopExportReport,
}

impl BuildDeliveryReport {
    pub fn delivery(&self) -> &DeliveryRef {
        &self.delivery
    }

    pub fn desktop_export(&self) -> &crate::DesktopExportReport {
        &self.desktop_export
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryVerificationReport {
    delivery: DeliveryRef,
    process_verification: ExportedPlayerProcessVerificationReport,
}

impl DeliveryVerificationReport {
    pub fn delivery(&self) -> &DeliveryRef {
        &self.delivery
    }

    pub fn process_verification(&self) -> &ExportedPlayerProcessVerificationReport {
        &self.process_verification
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameProjectCompilerStage {
    Bind,
    Check,
    Prepare,
    Run,
    Build,
    Verify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameProjectCompilerError {
    code: String,
    message: String,
    stage: GameProjectCompilerStage,
    next_action: String,
    source_location: Option<SourceLocation>,
    diagnostics: Vec<CheckDiagnostic>,
}

impl GameProjectCompilerError {
    pub fn source_location(&self) -> Option<&SourceLocation> {
        self.source_location.as_ref()
    }

    pub fn diagnostics(&self) -> &[CheckDiagnostic] {
        &self.diagnostics
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn stage(&self) -> GameProjectCompilerStage {
        self.stage
    }

    pub fn next_action(&self) -> &str {
        &self.next_action
    }
}

impl fmt::Display for GameProjectCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GameProjectCompilerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompilerOperationBinding {
    project_identity: String,
    opened_project_binding: String,
    revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameProjectCompiler {
    binding: CompilerOperationBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckProfile {
    target_profile: TargetProfile,
    cargo_executable: PathBuf,
    engine_sdk_root: PathBuf,
    timeout_ms: u64,
    process_approved: bool,
}

impl CheckProfile {
    pub fn new(target_profile: TargetProfile) -> Self {
        Self {
            target_profile,
            cargo_executable: std::env::var_os("CARGO")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("cargo")),
            engine_sdk_root: crate::default_engine_sdk_root(),
            timeout_ms: 120_000,
            process_approved: false,
        }
    }

    /// The embedding host must grant process execution before checking project Rust.
    pub fn with_process_approval(mut self, approved: bool) -> Self {
        self.process_approved = approved;
        self
    }

    pub fn with_cargo_executable(mut self, executable: PathBuf) -> Self {
        self.cargo_executable = executable;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms.clamp(1, 600_000);
        self
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub schema_version: &'static str,
    project_identity: String,
    #[serde(rename = "projectRevision")]
    revision_id: String,
    target_profile: TargetProfile,
    source_file_count: usize,
    pub diagnostics: Vec<CheckDiagnostic>,
    pub rust_check: String,
    pub checked_scope: Vec<String>,
}

impl CheckReport {
    pub fn project_identity(&self) -> &str {
        &self.project_identity
    }

    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub fn source_file_count(&self) -> usize {
        self.source_file_count
    }
}

impl GameProjectCompiler {
    pub fn bind(lease: &ProjectSnapshotLease) -> Result<Self, GameProjectCompilerError> {
        if lease.retention_policy() != SnapshotRetentionPolicy::OperationBound {
            return Err(compiler_error(
                "game_project_compiler.snapshot_retention_unsupported",
                "Game Project Compiler requires an operation-bound immutable snapshot lease.",
                GameProjectCompilerStage::Bind,
                "Acquire an operation-bound snapshot lease from AuthoringProjectContext.",
            ));
        }
        validate_snapshot_contract(lease.snapshot(), GameProjectCompilerStage::Bind)?;
        Ok(Self {
            binding: binding_from_snapshot(lease.snapshot()),
        })
    }

    pub fn check(
        &self,
        lease: &ProjectSnapshotLease,
        profile: CheckProfile,
    ) -> Result<CheckReport, GameProjectCompilerError> {
        let snapshot = lease.snapshot();
        validate_snapshot_contract(snapshot, GameProjectCompilerStage::Check)?;
        self.ensure_binding(snapshot, GameProjectCompilerStage::Check)?;
        let source_view = CompilerSourceView::from_snapshot(snapshot).map_err(|mut error| {
            error.stage = GameProjectCompilerStage::Check;
            error
        })?;
        if profile.target_profile != TargetProfile::WindowsDev || !cfg!(windows) {
            return Err(check::error(
                "check_target_unsupported",
                "Check is qualified only for Windows Dev on Windows.",
                None,
            ));
        }
        let manifest = check::validate_source(
            &source_view,
            snapshot,
            profile.target_profile,
            GameProjectCompilerStage::Check,
        )?;
        crate::particle_effect_cook::cook(&source_view).map_err(|mut failure| {
            failure.stage = GameProjectCompilerStage::Check;
            failure
        })?;
        let (diagnostics, rust_check) = check::check_rust(&source_view, &manifest, &profile)?;
        Ok(CheckReport {
            schema_version: "game-project-check.v1",
            project_identity: snapshot.portable_project_identity.clone(),
            revision_id: snapshot.revision.revision_id.clone(),
            target_profile: profile.target_profile,
            source_file_count: source_view.len(),
            diagnostics,
            rust_check,
            checked_scope: [
                "manifest",
                "build_profile",
                "authoring_json",
                "source_file_references",
                "rust_library_and_generated_glue",
            ]
            .map(str::to_string)
            .to_vec(),
        })
    }

    pub fn prepare(
        &self,
        lease: &ProjectSnapshotLease,
        target_profile: TargetProfile,
    ) -> Result<PreparedRuntimePackage, GameProjectCompilerError> {
        self.prepare_inner(lease, target_profile, None)
    }

    pub fn prepare_with_artifact_cache(
        &self,
        lease: &ProjectSnapshotLease,
        target_profile: TargetProfile,
        artifact_cache_root: &Path,
    ) -> Result<PreparedRuntimePackage, GameProjectCompilerError> {
        self.prepare_inner(lease, target_profile, Some(artifact_cache_root))
    }

    fn prepare_inner(
        &self,
        lease: &ProjectSnapshotLease,
        target_profile: TargetProfile,
        artifact_cache_root: Option<&Path>,
    ) -> Result<PreparedRuntimePackage, GameProjectCompilerError> {
        let snapshot = lease.snapshot();
        validate_snapshot_contract(snapshot, GameProjectCompilerStage::Prepare)?;
        self.ensure_binding(snapshot, GameProjectCompilerStage::Prepare)?;
        let source_view = CompilerSourceView::from_snapshot(snapshot)?;
        let manifest = check::validate_source(
            &source_view,
            snapshot,
            target_profile,
            GameProjectCompilerStage::Prepare,
        )?;
        if snapshot.qualification != ProjectQualification::Ready {
            return Err(compiler_error(
                "game_project_compiler.project_invalid",
                "The snapshot revision is invalid and cannot be prepared.",
                GameProjectCompilerStage::Prepare,
                "Repair the AuthoringProjectContext diagnostics, refresh, and acquire a new snapshot lease.",
            ));
        }
        let source_identity = source_identity(snapshot);
        let generated_runtime_glue = crate::generated_runtime_glue::generate_runtime_glue(
            &source_view,
            manifest.runtime_module.as_ref(),
            target_profile,
        )
        .map_err(|error| {
            compiler_error(
                error.code,
                error.message,
                GameProjectCompilerStage::Prepare,
                error.next_action,
            )
        })?;
        let (mut runtime_package_build_input, mut assembly_digest, producer_reports) =
            crate::neutral_assembler::assemble(&source_view, artifact_cache_root)?;
        if let Some(glue) = &generated_runtime_glue {
            runtime_package_build_input
                .project
                .runtime_module
                .aot_content_digest
                .clone_from(&glue.report().runtime_module_source_identity);
            assembly_digest = runtime_package_build_input
                .assembly_input_digest()
                .map_err(|error| {
                    compiler_error(
                        "game_project_compiler.generated_runtime_identity_failed",
                        error.to_string(),
                        GameProjectCompilerStage::Prepare,
                        "Repair the project RuntimeModule source and prepare it again.",
                    )
                })?
                .0
                .prefixed_value();
        }
        let lineage = ProjectArtifactLineage {
            project_identity: snapshot.portable_project_identity.clone(),
            opened_project_binding: snapshot.opened_project_binding.clone(),
            revision_id: snapshot.revision.revision_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            target_profile,
            source_identity,
        };
        let preparation_identity = preparation_identity(
            &lineage,
            generated_runtime_glue
                .as_ref()
                .map(crate::PreparedRuntimeGlue::generation_digest),
        );
        Ok(PreparedRuntimePackage {
            lineage,
            preparation_identity,
            source_file_count: source_view.len(),
            source: source_view,
            runtime_package_identity: assembly_digest.clone(),
            runtime_package_build_input,
            assembly_digest,
            producer_reports,
            generated_runtime_glue,
        })
    }

    pub fn run(
        &self,
        prepared: &PreparedRuntimePackage,
        options: RunOptions,
    ) -> Result<RuntimeExecutionReport, GameProjectCompilerError> {
        self.ensure_lineage(&prepared.lineage, GameProjectCompilerStage::Run)?;
        if options.mode != ExecutionMode::Headless {
            return Err(compiler_error(
                "game_project_compiler.run_mode_unsupported",
                "The Headless Compiler run owner currently supports only headless mode.",
                GameProjectCompilerStage::Run,
                "Use mode=headless for the qualified No-Editor run path.",
            ));
        }
        let timeout_ms = options.timeout_ms.max(1);
        let project_root = options.project_root.ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Run, "project root")
        })?;
        let output = options.project_relative_output.ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Run, "safe output path")
        })?;
        let player_build_root = options.player_artifact_build_root.ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Run, "Player build root")
        })?;
        let build = self.build(
            prepared,
            BuildRequest::for_project(
                prepared.lineage.target_profile,
                project_root,
                output,
                player_build_root,
            )
            .with_frame_limit(options.frame_limit.max(1))
            .with_timeout_ms(timeout_ms)
            .with_player_verification(options.verify_player),
        )?;
        Ok(RuntimeExecutionReport {
            lineage: prepared.lineage.clone(),
            preparation_identity: prepared.preparation_identity.clone(),
            delivery: build.delivery.clone(),
            desktop_export: build.desktop_export,
        })
    }

    pub fn build(
        &self,
        prepared: &PreparedRuntimePackage,
        request: BuildRequest,
    ) -> Result<BuildDeliveryReport, GameProjectCompilerError> {
        self.ensure_lineage(&prepared.lineage, GameProjectCompilerStage::Build)?;
        if prepared.lineage.target_profile != request.target_profile {
            return Err(compiler_error(
                "game_project_compiler.target_profile_mismatch",
                "The build request target profile does not match the prepared source identity.",
                GameProjectCompilerStage::Build,
                "Prepare the project again for the requested target profile.",
            ));
        }
        if request.target_profile != TargetProfile::WindowsDev {
            return Err(compiler_error(
                "game_project_compiler.build_target_not_ready",
                "Only the existing Windows Dev delivery owner is ready on the Headless Compiler.",
                GameProjectCompilerStage::Build,
                "Use targetProfile=windows-dev; other targets remain unavailable.",
            ));
        }
        let project_root = request.project_root.as_deref().ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Build, "project root")
        })?;
        let output = request.project_relative_output.clone().ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Build, "safe output path")
        })?;
        let player_build_root = request.player_artifact_build_root.clone().ok_or_else(|| {
            execution_binding_missing(GameProjectCompilerStage::Build, "Player build root")
        })?;
        validate_live_project_binding(
            project_root,
            &prepared.lineage,
            GameProjectCompilerStage::Build,
        )?;
        let mut desktop_request = crate::DesktopExportRequest::windows_dev(project_root)
            .with_project_relative_output(output)
            .with_player_artifact_build_root(player_build_root)
            .with_player_timeout_ms(request.timeout_ms)
            .with_player_verification(request.verify_player);
        desktop_request.frame_limit = request.frame_limit.max(1);
        let desktop_export =
            crate::DesktopExportPipeline::export_prepared(desktop_request, prepared);
        if desktop_export.status != crate::DesktopExportStatus::Success {
            let detail = desktop_export
                .diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .unwrap_or_else(|| "desktop export failed without a diagnostic".to_string());
            return Err(compiler_error(
                "game_project_compiler.desktop_export_failed",
                detail,
                GameProjectCompilerStage::Build,
                "Read the project-contained desktop export reports and repair the first failing stage.",
            ));
        }
        validate_live_project_binding(
            project_root,
            &prepared.lineage,
            GameProjectCompilerStage::Build,
        )?;
        let artifact_identity = desktop_export
            .player_artifact_hash
            .clone()
            .unwrap_or_else(|| prepared.runtime_package_identity.clone());
        // The export owner has already validated these file identities. Retain the
        // exact manifest bytes and its checked payload digest without another scan.
        let manifest_bytes =
            std::fs::read(&desktop_export.package_manifest_path).map_err(|error| {
                compiler_error(
                    "game_project_compiler.delivery_manifest_invalid",
                    error.to_string(),
                    GameProjectCompilerStage::Build,
                    "Repair the exported desktop manifest.",
                )
            })?;
        let manifest: runtime_cli::DesktopPackageManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| {
                compiler_error(
                    "game_project_compiler.delivery_manifest_invalid",
                    error.to_string(),
                    GameProjectCompilerStage::Build,
                    "Repair the exported desktop manifest.",
                )
            })?;
        let runtime_package_digest = manifest
            .runtime_package_digest
            .filter(|_| {
                manifest.player_artifact_hash.as_deref() == Some(artifact_identity.as_str())
                    && manifest.player_module_descriptor == desktop_export.player_module_descriptor
            })
            .ok_or_else(|| {
                compiler_error(
            "game_project_compiler.delivery_manifest_invalid",
            "Successful export manifest does not match its reported Player/package identity.",
            GameProjectCompilerStage::Build,
            "Repair the exported desktop manifest.",
        )
            })?;
        let manifest_digest = format!("sha256:{:x}", Sha256::digest(&manifest_bytes));
        let delivery_identity = delivery_identity(prepared, &desktop_export, &artifact_identity);
        let mut evidence_refs = vec![
            desktop_export.package_manifest_path.clone(),
            desktop_export.runtime_package_report_path.clone(),
        ];
        // A normal development build deliberately does not create the Player
        // process report. Publish only evidence that exists; explicit verify
        // adds its own report later.
        if std::path::Path::new(&desktop_export.player_report_path).exists() {
            evidence_refs.push(desktop_export.player_report_path.clone());
        }
        let delivery = DeliveryRef {
            lineage: prepared.lineage.clone(),
            preparation_identity: prepared.preparation_identity.clone(),
            delivery_identity,
            package_dir: PathBuf::from(&desktop_export.package_dir),
            artifact_identity,
            runtime_package_digest,
            manifest_digest,
            evidence_refs,
        };
        Ok(BuildDeliveryReport {
            delivery,
            desktop_export,
        })
    }

    pub fn verify(
        &self,
        delivery: &DeliveryRef,
        request: VerifyRequest,
    ) -> Result<DeliveryVerificationReport, GameProjectCompilerError> {
        self.ensure_lineage(&delivery.lineage, GameProjectCompilerStage::Verify)?;
        delivery.ensure_manifest_identity()?;
        let mode = match request.mode {
            ExecutionMode::Headless => "headless",
            ExecutionMode::Windowed => "windowed",
        };
        let report = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
            exported_package_dir: delivery.package_dir.clone(),
            mode: mode.to_string(),
            frame_limit: request.frame_limit.max(1),
            report_path: None,
            timeout_ms: request.timeout_ms.max(1),
            screenshot: request.screenshot,
            screenshot_path: None,
        });
        if report.status != ExportedPlayerProcessVerificationStatus::Passed
            || report.process_exit_code != Some(0)
            || report.child_player_exit_code != Some(0)
        {
            let detail = report
                .diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .unwrap_or_else(|| format!("Player verification ended as {:?}.", report.status));
            return Err(compiler_error(
                "game_project_compiler.delivery_verification_failed",
                detail,
                GameProjectCompilerStage::Verify,
                "Read exported-player-process-verification-report.json and repair the delivery.",
            ));
        }
        Ok(DeliveryVerificationReport {
            delivery: delivery.clone(),
            process_verification: report,
        })
    }

    fn ensure_binding(
        &self,
        snapshot: &ProjectSnapshot,
        stage: GameProjectCompilerStage,
    ) -> Result<(), GameProjectCompilerError> {
        let actual = binding_from_snapshot(snapshot);
        if self.binding == actual {
            return Ok(());
        }
        Err(binding_mismatch(stage))
    }

    fn ensure_lineage(
        &self,
        lineage: &ProjectArtifactLineage,
        stage: GameProjectCompilerStage,
    ) -> Result<(), GameProjectCompilerError> {
        if self.binding.project_identity == lineage.project_identity
            && self.binding.opened_project_binding == lineage.opened_project_binding
            && self.binding.revision_id == lineage.revision_id
        {
            return Ok(());
        }
        Err(binding_mismatch(stage))
    }
}

fn validate_snapshot_contract(
    snapshot: &ProjectSnapshot,
    stage: GameProjectCompilerStage,
) -> Result<(), GameProjectCompilerError> {
    let revision = &snapshot.revision;
    if snapshot.schema_version != PROJECT_SNAPSHOT_SCHEMA_VERSION
        || snapshot.snapshot_id.is_empty()
        || snapshot.portable_project_identity.is_empty()
        || snapshot.opened_project_binding.is_empty()
        || revision.revision_id.is_empty()
        || revision.portable_project_identity != snapshot.portable_project_identity
        || revision.qualification != snapshot.qualification
    {
        return Err(compiler_error(
            "game_project_compiler.snapshot_contract_invalid",
            "The snapshot lease does not satisfy the Game Project Compiler identity contract.",
            stage,
            "Acquire a new snapshot lease from the current AuthoringProjectContext revision.",
        ));
    }
    Ok(())
}

fn validate_lease_owned_sources(
    snapshot: &ProjectSnapshot,
) -> Result<(), GameProjectCompilerError> {
    if !snapshot
        .files
        .iter()
        .any(|file| file.relative_path == PROJECT_MANIFEST_PATH)
    {
        return Err(compiler_error(
            "game_project_compiler.manifest_missing_from_snapshot",
            "The snapshot lease does not own project.aife.json.",
            GameProjectCompilerStage::Prepare,
            "Acquire a compiler snapshot lease that includes the canonical project manifest.",
        ));
    }

    let mut previous_path: Option<&str> = None;
    for file in &snapshot.files {
        if previous_path.is_some_and(|previous| previous >= file.relative_path.as_str()) {
            return Err(compiler_error(
                "game_project_compiler.snapshot_source_order_invalid",
                "Snapshot source paths must be unique and in canonical order.",
                GameProjectCompilerStage::Prepare,
                "Acquire a fresh snapshot lease from AuthoringProjectContext.",
            ));
        }
        previous_path = Some(&file.relative_path);

        let actual_digest = format!("sha256:{:x}", Sha256::digest(&file.bytes));
        if file.length != file.bytes.len() as u64 || file.content_digest != actual_digest {
            return Err(compiler_error(
                "game_project_compiler.snapshot_source_identity_invalid",
                format!(
                    "Snapshot source identity does not match lease-owned bytes for '{}'.",
                    file.relative_path
                ),
                GameProjectCompilerStage::Prepare,
                "Discard the lease and acquire a fresh snapshot from AuthoringProjectContext.",
            ));
        }
    }
    Ok(())
}

fn binding_from_snapshot(snapshot: &ProjectSnapshot) -> CompilerOperationBinding {
    CompilerOperationBinding {
        project_identity: snapshot.portable_project_identity.clone(),
        opened_project_binding: snapshot.opened_project_binding.clone(),
        revision_id: snapshot.revision.revision_id.clone(),
    }
}

fn source_identity(snapshot: &ProjectSnapshot) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_SNAPSHOT_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(snapshot.snapshot_id.as_bytes());
    hasher.update([0]);
    for file in &snapshot.files {
        hasher.update((file.relative_path.len() as u64).to_le_bytes());
        hasher.update(file.relative_path.as_bytes());
        hasher.update(file.length.to_le_bytes());
        hasher.update(file.content_digest.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn preparation_identity(
    lineage: &ProjectArtifactLineage,
    generated_runtime_glue_digest: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PREPARED_RUNTIME_PACKAGE_IDENTITY_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(lineage.project_identity.as_bytes());
    hasher.update([0]);
    hasher.update(lineage.opened_project_binding.as_bytes());
    hasher.update([0]);
    hasher.update(lineage.revision_id.as_bytes());
    hasher.update([0]);
    hasher.update(lineage.snapshot_id.as_bytes());
    hasher.update([0]);
    hasher.update(lineage.target_profile.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(lineage.source_identity.as_bytes());
    hasher.update([0]);
    hasher.update(generated_runtime_glue_digest.unwrap_or("none").as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn delivery_identity(
    prepared: &PreparedRuntimePackage,
    report: &crate::DesktopExportReport,
    artifact_identity: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"game-project-delivery-identity.v1\0");
    hasher.update(prepared.preparation_identity.as_bytes());
    hasher.update([0]);
    hasher.update(report.package_dir.as_bytes());
    hasher.update([0]);
    hasher.update(artifact_identity.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn validate_live_project_binding(
    project_root: &Path,
    lineage: &ProjectArtifactLineage,
    stage: GameProjectCompilerStage,
) -> Result<(), GameProjectCompilerError> {
    let session = crate::ProjectAuthoringSession::open(project_root).map_err(|error| {
        compiler_error(
            "game_project_compiler.execution_project_open_failed",
            error.to_string(),
            stage,
            "Use the project root bound to the same provider session and retry.",
        )
    })?;
    let revision = session.revision().map_err(|error| {
        compiler_error(
            "game_project_compiler.execution_project_revision_failed",
            error.to_string(),
            stage,
            "Refresh the canonical project and retry with a new operation.",
        )
    })?;
    if revision.portable_project_identity != lineage.project_identity
        || revision.revision_id != lineage.revision_id
    {
        return Err(compiler_error(
            "game_project_compiler.execution_project_drifted",
            "The live project no longer matches the prepared snapshot revision.",
            stage,
            "Discard the prepared artifact, refresh, and restart the operation from prepare.",
        ));
    }
    Ok(())
}

fn binding_mismatch(stage: GameProjectCompilerStage) -> GameProjectCompilerError {
    compiler_error(
        "game_project_compiler.operation_binding_mismatch",
        "The source or artifact belongs to another project binding or revision.",
        stage,
        "Use a compiler bound to the same canonical project and snapshot revision.",
    )
}

fn execution_binding_missing(
    stage: GameProjectCompilerStage,
    binding: &str,
) -> GameProjectCompilerError {
    compiler_error(
        "game_project_compiler.execution_binding_missing",
        format!("Compiler execution requires an owner-provided {binding}."),
        stage,
        "Invoke this operation through ProjectAuthoringSession or EngineToolProvider.",
    )
}

fn compiler_error(
    code: impl Into<String>,
    message: impl Into<String>,
    stage: GameProjectCompilerStage,
    next_action: impl Into<String>,
) -> GameProjectCompilerError {
    GameProjectCompilerError {
        code: code.into(),
        message: message.into(),
        stage,
        next_action: next_action.into(),
        source_location: None,
        diagnostics: Vec::new(),
    }
}

impl GameProjectCompilerError {
    pub(crate) fn with_source_location(mut self, location: SourceLocation) -> Self {
        self.source_location = Some(location.clone());
        self.diagnostics.push(CheckDiagnostic {
            code: self.code.clone(),
            severity: "error".into(),
            message: self.message.clone(),
            location: Some(location),
            next_action: self.next_action.clone(),
        });
        self
    }

    pub(crate) fn new_for_assembly(code: &'static str, message: String) -> Self {
        compiler_error(
            format!("game_project_compiler.{code}"),
            message,
            GameProjectCompilerStage::Prepare,
            "Repair the leased canonical source and acquire a new snapshot lease.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use authoring_project_context::SnapshotFile;

    #[test]
    fn run_and_build_verification_defaults_remain_enabled_with_explicit_opt_out() {
        let run = RunOptions::new(ExecutionMode::Headless, 1, 30_000);
        assert!(run.verify_player);
        assert!(!run.with_player_verification(false).verify_player);
        let build = BuildRequest::new(TargetProfile::WindowsDev);
        assert!(build.verify_player);
        assert!(!build.with_player_verification(false).verify_player);
    }

    #[test]
    fn game_project_compiler_source_view_reads_only_lease_owned_bytes() {
        let manifest = br#"{"schemaVersion":"aife-project.v2","projectId":"project-1"}"#.to_vec();
        let source = b"fn game() { 1 }".to_vec();
        let snapshot = ProjectSnapshot {
            schema_version: PROJECT_SNAPSHOT_SCHEMA_VERSION.to_string(),
            snapshot_id: "snapshot-1".to_string(),
            portable_project_identity: "project-1".to_string(),
            opened_project_binding: "binding-1".to_string(),
            revision: authoring_project_context::ProjectRevision {
                schema_version: "project-revision.v1".to_string(),
                portable_project_identity: "project-1".to_string(),
                source_policy_version: "project-source-policy.v1".to_string(),
                source_digest: "sha256:source".to_string(),
                revision_id: "revision-1".to_string(),
                qualification: authoring_project_context::ProjectQualification::Ready,
                diagnostics_digest: "sha256:diagnostics".to_string(),
            },
            qualification: authoring_project_context::ProjectQualification::Ready,
            files: vec![
                SnapshotFile {
                    relative_path: "Game/main.rs".to_string(),
                    length: source.len() as u64,
                    content_digest: format!("sha256:{:x}", Sha256::digest(&source)),
                    bytes: source.clone(),
                },
                SnapshotFile {
                    relative_path: PROJECT_MANIFEST_PATH.to_string(),
                    length: manifest.len() as u64,
                    content_digest: format!("sha256:{:x}", Sha256::digest(&manifest)),
                    bytes: manifest.clone(),
                },
            ],
        };

        let view = CompilerSourceView::from_snapshot(&snapshot).expect("valid source view");
        assert_eq!(view.len(), 2);
        assert_eq!(view.bytes(PROJECT_MANIFEST_PATH), Some(manifest.as_slice()));
        assert_eq!(view.bytes("Game/main.rs"), Some(source.as_slice()));
        assert_eq!(view.bytes("Game/live.rs"), None);
        assert_eq!(
            view.project_manifest_value().unwrap()["projectId"],
            "project-1"
        );
    }

    #[test]
    fn game_project_compiler_interface_delivery_identity_is_revision_bound() {
        let compiler = GameProjectCompiler {
            binding: CompilerOperationBinding {
                project_identity: "project-a".to_string(),
                opened_project_binding: "binding-a".to_string(),
                revision_id: "revision-a".to_string(),
            },
        };
        let delivery = DeliveryRef {
            lineage: ProjectArtifactLineage {
                project_identity: "project-b".to_string(),
                opened_project_binding: "binding-b".to_string(),
                revision_id: "revision-b".to_string(),
                snapshot_id: "snapshot-b".to_string(),
                target_profile: TargetProfile::WindowsDev,
                source_identity: "source-b".to_string(),
            },
            preparation_identity: "prepared-b".to_string(),
            delivery_identity: "delivery-b".to_string(),
            package_dir: PathBuf::from("delivery-b"),
            artifact_identity: "artifact-b".to_string(),
            runtime_package_digest: String::new(),
            manifest_digest: String::new(),
            evidence_refs: Vec::new(),
        };

        let error = compiler
            .verify(
                &delivery,
                VerifyRequest::new(ExecutionMode::Headless, 1, 1_000, false),
            )
            .expect_err("cross-project delivery must fail before verification");

        assert_eq!(
            error.code(),
            "game_project_compiler.operation_binding_mismatch"
        );
        assert_eq!(error.stage(), GameProjectCompilerStage::Verify);
    }

    #[test]
    fn game_project_compiler_verify_rejects_coherent_dll_manifest_replacement() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "aife-delivery-manifest-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(root.join("data/runtime_package")).unwrap();
        fs::create_dir_all(root.join("data/bin")).unwrap();
        let descriptor =
            engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                "fixture.delivery.runtime",
                "sha256:fixture-project",
            );
        let module_path = root.join(runtime_cli::project_runtime_module_relative_path(
            &descriptor.module_id,
        ));
        fs::write(root.join("Game.exe"), b"fixture-host").unwrap();
        fs::write(root.join("engine_runtime.dll"), b"original-engine").unwrap();
        fs::write(&module_path, b"fixture-project").unwrap();
        fs::write(
            root.join("data/runtime_package/manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": "runtime-package.v2",
                "project": {"runtimeModule": descriptor},
            }))
            .unwrap(),
        )
        .unwrap();
        let mut manifest = runtime_cli::DesktopPackageManifest {
            schema_version: runtime_cli::DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
            target: "windows".to_string(),
            profile: "dev".to_string(),
            package_dir: root.display().to_string(),
            runtime_package_dir: root.join("data/runtime_package").display().to_string(),
            reports_dir: root.join("reports").display().to_string(),
            player_executable: Some(root.join("Game.exe").display().to_string()),
            player_executable_status: "copied".to_string(),
            player_artifact_build_report_path: None,
            player_artifact_hash: Some(
                runtime_cli::semantic_file_digest(&root.join("Game.exe")).unwrap(),
            ),
            player_module_descriptor: Some(descriptor),
            engine_runtime_hash: Some(
                runtime_cli::semantic_file_digest(&root.join("engine_runtime.dll")).unwrap(),
            ),
            project_runtime_module_hash: Some(
                runtime_cli::semantic_file_digest(&module_path).unwrap(),
            ),
            runtime_package_digest: Some(
                runtime_cli::runtime_package_digest(&root.join("data/runtime_package")).unwrap(),
            ),
        };
        let manifest_path = root.join("package-manifest.json");
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        runtime_cli::validate_desktop_dev_package(&root).unwrap();
        let compiler = GameProjectCompiler {
            binding: CompilerOperationBinding {
                project_identity: "fixture-project".to_string(),
                opened_project_binding: "fixture-binding".to_string(),
                revision_id: "fixture-revision".to_string(),
            },
        };
        let delivery = DeliveryRef {
            lineage: ProjectArtifactLineage {
                project_identity: compiler.binding.project_identity.clone(),
                opened_project_binding: compiler.binding.opened_project_binding.clone(),
                revision_id: compiler.binding.revision_id.clone(),
                snapshot_id: "fixture-snapshot".to_string(),
                target_profile: TargetProfile::WindowsDev,
                source_identity: "fixture-source".to_string(),
            },
            preparation_identity: "fixture-prepared".to_string(),
            delivery_identity: "fixture-delivery".to_string(),
            package_dir: root.clone(),
            artifact_identity: manifest.player_artifact_hash.clone().unwrap(),
            runtime_package_digest: manifest.runtime_package_digest.clone().unwrap(),
            manifest_digest: runtime_cli::semantic_file_digest(&manifest_path).unwrap(),
            evidence_refs: Vec::new(),
        };
        fs::write(root.join("engine_runtime.dll"), b"replacement-engine").unwrap();
        manifest.engine_runtime_hash =
            Some(runtime_cli::semantic_file_digest(&root.join("engine_runtime.dll")).unwrap());
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        runtime_cli::validate_desktop_dev_package(&root).unwrap();

        let error = compiler
            .verify(
                &delivery,
                VerifyRequest::new(ExecutionMode::Headless, 1, 1_000, false),
            )
            .unwrap_err();
        assert_eq!(error.code(), "game_project_compiler.delivery_changed");
        assert!(error.message().contains("package-manifest.json"));
        assert!(
            !root.join("reports").exists(),
            "rejection must precede process verification"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
