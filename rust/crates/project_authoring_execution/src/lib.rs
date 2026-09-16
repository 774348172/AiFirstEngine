use authoring_project_context::{
    CanonicalSourceInventory, ContextError, DocumentWriteReport, DocumentWriteRequest,
    EmbeddedAuthoringProjectContext, OpenOptions, ProjectHandle, ProjectLocator, ProjectMutation,
    ProjectMutationReceipt, ProjectMutationRollbackReceipt, ProjectRecoveryReport, ProjectRevision,
    ProjectSnapshot, ProjectSnapshotLease, RefreshReport, SnapshotLeaseRequest, SnapshotRequest,
    SourceInventoryEntry,
};
use std::path::{Path, PathBuf};

mod artifact_cache;
mod builtin_font_pack;
mod desktop_export;
mod font_assets;
mod font_bundle;
mod font_cook;
mod game_project_compiler;
mod generated_runtime_glue;
mod neutral_assembler;
mod particle_effect_cook;
mod project_player_artifact;
mod project_runtime_package_assembler;
mod project_runtime_player_staging;
mod project_schema;
mod project_write_scope;

pub use artifact_cache::{
    ProjectAssemblyArtifactCache, ProjectAssemblyArtifactCacheError,
    ProjectAssemblyArtifactCacheStatus, ProjectAssemblyArtifactEnvelope,
    ProjectAssemblyArtifactLookup, ProjectAssemblyArtifactPublishResult,
    ProjectAssemblyArtifactPublishStatus, ProjectAssemblyProducerReport,
    ProjectAssemblyProducerSubstageReport, PROJECT_ASSEMBLY_ARTIFACT_ENVELOPE_SCHEMA_VERSION,
    PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION,
};
pub use builtin_font_pack::{
    load_engine_builtin_font_pack, EngineBuiltInFontPackError, EngineBuiltInFontPackManifest,
    ENGINE_BUILT_IN_FONT_PACK_ID, ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION,
};
pub use font_assets::*;
pub use font_bundle::{
    select_auto_hybrid, FontAutoHybridDecision, FontAutoHybridRequest, ProjectFontBundleBuilder,
};
pub use font_cook::*;

pub use desktop_export::{
    default_player_executable_for_project, DesktopExportDiagnostic,
    DesktopExportDiagnosticSeverity, DesktopExportPipeline, DesktopExportReport,
    DesktopExportRequest, DesktopExportStatus, DesktopExportTarget, DesktopPackageManifest,
    ExplicitExportOutput, DESKTOP_EXPORT_REPORT_SCHEMA_VERSION,
    DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION,
};

pub use game_project_compiler::{
    BuildDeliveryReport, BuildRequest, CheckDiagnostic, CheckProfile, CheckReport, DeliveryRef,
    DeliveryVerificationReport, ExecutionMode, GameProjectCompiler, GameProjectCompilerError,
    GameProjectCompilerStage, PreparedPlaytestScenario, PreparedRuntimePackage,
    ProjectArtifactLineage, ProjectPlaytestReport, RunOptions, RuntimeExecutionReport,
    SourceLocation, TargetProfile, VerifyRequest,
};
pub use generated_runtime_glue::{
    GeneratedRuntimeGlueMaterializationError, GeneratedRuntimeGlueReport,
    GeneratedRuntimeGlueSourceMapEntry, PreparedRuntimeGlue,
    GENERATED_RUNTIME_GLUE_REPORT_SCHEMA_VERSION,
};
#[doc(hidden)]
pub use project_player_artifact::runtime_module_source_digest;
pub use project_player_artifact::{
    default_engine_sdk_root, default_project_runtime_player_build_root, ProjectPlayerArtifact,
    ProjectPlayerArtifactError, ProjectRuntimePlayerArtifactBuildDiagnostic,
    ProjectRuntimePlayerArtifactBuildReport, ProjectRuntimePlayerArtifactBuildRequest,
    ProjectRuntimePlayerArtifactBuildStatus, ProjectRuntimePlayerArtifactBuildStep,
    PROJECT_PLAYER_ARTIFACT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION,
};
pub use project_runtime_package_assembler::{
    BuildProfile, BuildProfileApplication, BuildProfileIconRef, BuildProfileRelease,
    BuildProfileValidationIssue, PrefabRuntimeBakeInstanceEntry, PrefabRuntimeBakeReport,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyDiagnostic,
    ProjectRuntimePackageAssemblyDomain, ProjectRuntimePackageAssemblyReport,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyResult,
    ProjectRuntimePackageAssemblySeverity, ProjectRuntimePackageAssemblyStatus,
    ProjectRuntimeSourceMapping, BUILD_PROFILE_SCHEMA_VERSION, BUILD_PROFILE_SCHEMA_VERSION_V1,
    PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION,
};
#[doc(hidden)]
pub use project_runtime_player_staging::{
    ProjectRuntimePlayerDependencyIdentity, ProjectRuntimePlayerStagingError,
    ProjectRuntimePlayerStagingPlan, ProjectRuntimeProductionStaging,
};
pub use project_schema::{
    ProjectManifest, ProjectRuntimeModuleBuildSpec, ProjectRuntimeSourceKind,
    LEGACY_PROJECT_MANIFEST_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION,
    PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
};
pub use project_write_scope::{
    ProjectDirectoryWriter, ProjectRelativePath, ProjectWriteError, ProjectWriteOperation,
    ProjectWriteOutcome, ProjectWriteReceipt, ProjectWriteScope,
};

#[derive(Debug)]
pub struct ProjectAuthoringSession {
    context: EmbeddedAuthoringProjectContext,
    handle: ProjectHandle,
    project_root: PathBuf,
    recovery: ProjectRecoveryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSourceInventory {
    pub source_digest: String,
    pub entries: Vec<SourceInventoryEntry>,
}

impl ProjectAuthoringSession {
    pub fn open(project_root: impl AsRef<Path>) -> Result<Self, ContextError> {
        let mut context = EmbeddedAuthoringProjectContext::new();
        let open =
            context.open_with_report(ProjectLocator::new(project_root.as_ref()), OpenOptions)?;
        let project_root = context.project_root(open.handle)?;
        Ok(Self {
            context,
            handle: open.handle,
            project_root,
            recovery: open.recovery,
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn recovery_report(&self) -> &ProjectRecoveryReport {
        &self.recovery
    }

    pub fn revision(&self) -> Result<ProjectRevision, ContextError> {
        self.context.current_revision(self.handle)
    }

    pub fn refresh(&mut self) -> Result<RefreshReport, ContextError> {
        self.context.refresh(self.handle)
    }

    pub fn source_inventory(&self) -> Result<ProjectSourceInventory, ContextError> {
        let inventory = CanonicalSourceInventory::capture(&self.project_root)?;
        Ok(ProjectSourceInventory {
            source_digest: inventory.source_digest().to_string(),
            entries: inventory.entries().to_vec(),
        })
    }

    pub fn snapshot(&self, relative_paths: Vec<String>) -> Result<ProjectSnapshot, ContextError> {
        self.context
            .snapshot(self.handle, SnapshotRequest { relative_paths })
    }

    pub fn acquire_snapshot_lease(
        &mut self,
        owner: impl Into<String>,
        relative_paths: Vec<String>,
    ) -> Result<ProjectSnapshotLease, ContextError> {
        self.context.acquire_snapshot_lease(
            self.handle,
            SnapshotLeaseRequest {
                owner: owner.into(),
                snapshot: SnapshotRequest { relative_paths },
            },
        )
    }

    pub fn save_document(
        &mut self,
        request: DocumentWriteRequest,
    ) -> Result<DocumentWriteReport, ContextError> {
        self.context.save_document(self.handle, request)
    }

    pub fn capture_mutation_before(
        &self,
        paths: &[String],
    ) -> Result<Vec<authoring_project_context::ProjectMutationBeforeState>, ContextError> {
        self.context.capture_mutation_before(self.handle, paths)
    }

    pub fn commit_mutation(
        &mut self,
        mutation: ProjectMutation,
    ) -> Result<ProjectMutationReceipt, ContextError> {
        self.context.commit_mutation(self.handle, mutation)
    }

    pub fn rollback_mutation(
        &mut self,
        receipt: &ProjectMutationReceipt,
    ) -> Result<ProjectMutationRollbackReceipt, ContextError> {
        self.context.rollback_mutation(self.handle, receipt)
    }

    pub fn mutation_receipt_for_journal(
        &self,
        journal_relative_path: &str,
    ) -> Result<ProjectMutationReceipt, ContextError> {
        self.context
            .mutation_receipt_for_journal(self.handle, journal_relative_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use authoring_project_context::{
        ProjectMutationBeforeState, ProjectMutationOperation, ProjectQualification,
        PROJECT_MUTATION_SCHEMA_VERSION,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn refresh_direct_file_changes_without_editor() {
        let fixture = Fixture::new("refresh");
        let mut session = ProjectAuthoringSession::open(fixture.path()).unwrap();
        let first = session.revision().unwrap();
        fs::write(fixture.path().join("game.rs"), b"fn game() {}").unwrap();

        let refreshed = session.refresh().unwrap();

        assert_ne!(first.revision_id, refreshed.revision.revision_id);
        assert_eq!(
            refreshed.revision.qualification,
            ProjectQualification::Ready
        );
        assert_eq!(
            session.project_root(),
            fixture.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn operation_owned_lease_keeps_old_bytes_across_refresh() {
        let fixture = Fixture::new("lease");
        fs::write(fixture.path().join("game.rs"), b"old").unwrap();
        let mut session = ProjectAuthoringSession::open(fixture.path()).unwrap();
        let lease = session
            .acquire_snapshot_lease("operation-1", vec!["game.rs".to_string()])
            .unwrap();
        fs::write(fixture.path().join("game.rs"), b"new").unwrap();
        session.refresh().unwrap();

        assert_eq!(lease.snapshot().files[0].bytes, b"old");
        assert_ne!(
            lease.snapshot().revision.revision_id,
            session.revision().unwrap().revision_id
        );
        assert_eq!(
            lease.release().outcome,
            authoring_project_context::SnapshotReleaseOutcome::Released
        );
    }

    #[test]
    fn structured_commit_and_rollback_use_context_authority() {
        let fixture = Fixture::new("mutation");
        let mut session = ProjectAuthoringSession::open(fixture.path()).unwrap();
        let revision = session.revision().unwrap();
        let path = "Data/value.json".to_string();
        let before = session
            .capture_mutation_before(std::slice::from_ref(&path))
            .unwrap();
        assert_eq!(
            before,
            vec![ProjectMutationBeforeState {
                path: path.clone(),
                content_digest: None,
            }]
        );
        let receipt = session
            .commit_mutation(ProjectMutation {
                schema_version: PROJECT_MUTATION_SCHEMA_VERSION.to_string(),
                mutation_id: "provider-mutation".to_string(),
                domain: "project".to_string(),
                expected_revision_id: revision.revision_id,
                validation_digest: sha256_token("validated"),
                declared_read_set: Vec::new(),
                declared_write_set: vec![path.clone()],
                expected_before: before,
                operations: vec![ProjectMutationOperation::CreateOrReplace {
                    path: path.clone(),
                    bytes: b"{\"value\":1}".to_vec(),
                }],
            })
            .unwrap();
        assert_eq!(
            fs::read(fixture.path().join(&path)).unwrap(),
            b"{\"value\":1}"
        );

        let rollback = session.rollback_mutation(&receipt).unwrap();

        assert!(!fixture.path().join(path).exists());
        assert_eq!(
            rollback.restored_revision.revision_id,
            receipt.before_revision.revision_id
        );
    }

    fn sha256_token(label: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut value = std::collections::hash_map::DefaultHasher::new();
        label.hash(&mut value);
        format!("sha256:{:064x}", value.finish())
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "aife-project-authoring-execution-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("project.aife.json"),
                br#"{"schemaVersion":"aife-project.v2","projectId":"provider.fixture"}"#,
            )
            .unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
pub(crate) use game_project_compiler::CompilerSourceView;
