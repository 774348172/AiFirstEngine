use crate::{
    CandidateBaseVerificationStatus, CandidateFileChange, CandidateProjectRevision,
    CandidateProjectRevisionError, CandidateProjectRevisionRequest, CandidateProjectRevisionStore,
    ProjectManifest, ProjectRelativePath, ProjectWriteError, ProjectWriteScope,
    PROJECT_MANIFEST_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use url::Url;

pub const PROJECT_ASSET_IMPORT_CANDIDATE_SCHEMA_VERSION: &str = "project-asset-import-candidate.v1";
pub const PROJECT_ASSET_IMPORT_VALIDATION_REPORT_SCHEMA_VERSION: &str =
    "project-asset-import-validation-report.v1";
pub const PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION: &str = "project-asset-import-approval.v1";
pub const PROJECT_ASSET_IMPORT_APPLY_RECEIPT_SCHEMA_VERSION: &str =
    "project-asset-import-apply-receipt.v1";
pub const PROJECT_ASSET_IMPORT_ROLLBACK_RECEIPT_SCHEMA_VERSION: &str =
    "project-asset-import-rollback-receipt.v1";
pub const PROJECT_ASSET_META_SCHEMA_VERSION: &str = "project-asset-meta.v1";
pub const PROJECT_ASSET_DATABASE_SCHEMA_VERSION: &str = "project-asset-database.v1";
pub const PROJECT_ASSET_GRAPH_SCHEMA_VERSION: &str = "project-asset-graph.v1";
pub const PROJECT_ASSET_REGISTRY_SCHEMA_VERSION: &str = "project-asset-registry.v1";
const PROJECT_ASSET_IMPORT_ROLLBACK_RECORD_SCHEMA_VERSION: &str =
    "project-asset-import-rollback-record.v1";

const TEXTURE_IMPORTER_ID: &str = "texture.png.v1";
const TEXTURE_IMPORTER_VERSION: u32 = 1;
pub const FONT_SOURCE_ASSET_TYPE: &str = "fontSource";
pub const FONT_SOURCE_IMPORTER_ID: &str = "font.ttf.v2";
pub const FONT_SOURCE_IMPORTER_VERSION: u32 = 2;
const ASSET_DATABASE_PATH: &str = "Library/AssetPipeline/asset-database.json";
const ASSET_GRAPH_PATH: &str = "Library/AssetPipeline/asset-graph.json";
const ASSET_REGISTRY_PATH: &str = "Library/AssetPipeline/asset-registry.json";
const ASSET_IMPORT_LOCK_PATH: &str = "Library/AssetPipeline/import.lock";
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 256 * 1024 * 1024;
const MAX_TEXTURE_DIMENSION: u32 = 16_384;

type CapturedFileState = (String, Option<Vec<u8>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetImportSourceKind {
    LocalFile,
    AiGenerated,
    Downloaded,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetImportSourceMetadata {
    pub kind: AssetImportSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl AssetImportSourceMetadata {
    pub fn local_file() -> Self {
        Self {
            kind: AssetImportSourceKind::LocalFile,
            source_uri: None,
            creator: None,
            note: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetLicenseKind {
    ProjectOwned,
    ThirdParty,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetLicenseMetadata {
    pub kind: AssetLicenseKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl AssetLicenseMetadata {
    pub fn project_owned() -> Self {
        Self {
            kind: AssetLicenseKind::ProjectOwned,
            identifier: Some("project-owned".to_string()),
            license_uri: None,
            attribution: None,
            note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum AssetImportConflictPolicy {
    RejectExisting,
    ReplaceMatching {
        expected_asset_guid: String,
        expected_source_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextureImportSettings {
    pub color_space: String,
    pub sampler: String,
}

impl Default for TextureImportSettings {
    fn default() -> Self {
        Self {
            color_space: "srgb".to_string(),
            sampler: "linearClamp".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportPrepareRequest {
    pub import_id: String,
    pub revision_id: String,
    pub project_root: PathBuf,
    pub candidate_store_root: PathBuf,
    pub source_path: PathBuf,
    pub target_directory: String,
    pub asset_id: String,
    pub display_name: String,
    pub conflict_policy: AssetImportConflictPolicy,
    pub source_metadata: AssetImportSourceMetadata,
    pub license: AssetLicenseMetadata,
    pub texture_settings: TextureImportSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetDatabaseRecordState {
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetDatabaseRecord {
    pub asset_guid: String,
    pub asset_id: String,
    pub display_name: String,
    pub asset_type: String,
    pub descriptor_path: String,
    pub source_path: String,
    pub meta_path: String,
    pub source_hash: String,
    pub source_byte_length: u64,
    pub importer_id: String,
    pub importer_version: u32,
    pub settings_hash: String,
    pub state: AssetDatabaseRecordState,
    pub source_metadata: AssetImportSourceMetadata,
    pub license: AssetLicenseMetadata,
    pub direct_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetDatabaseDocument {
    pub schema_version: String,
    pub project_id: String,
    pub database_version: u64,
    pub assets: Vec<AssetDatabaseRecord>,
}

impl AssetDatabaseDocument {
    fn empty(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_ASSET_DATABASE_SCHEMA_VERSION.to_string(),
            project_id: project_id.into(),
            database_version: 0,
            assets: Vec::new(),
        }
    }

    pub fn asset_by_id(&self, asset_id: &str) -> Option<&AssetDatabaseRecord> {
        self.assets
            .iter()
            .find(|record| record.asset_id == asset_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetGraphNode {
    pub asset_guid: String,
    pub asset_id: String,
    pub direct_dependencies: Vec<String>,
    pub source_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetGraphDocument {
    pub schema_version: String,
    pub built_from_database_version: u64,
    pub nodes: Vec<AssetGraphNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetRegistryEntry {
    pub asset_guid: String,
    pub asset_id: String,
    pub asset_type: String,
    pub descriptor_path: String,
    pub source_path: String,
    pub meta_path: String,
    pub source_hash: String,
    pub importer_id: String,
    pub importer_version: u32,
    pub direct_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetRegistryDocument {
    pub schema_version: String,
    pub registry_version: u64,
    pub built_from_database_version: u64,
    pub entries: Vec<AssetRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetMeta {
    pub schema_version: String,
    pub asset_guid: String,
    pub asset_id: String,
    pub asset_type: String,
    pub descriptor_path: String,
    pub source_path: String,
    pub source_hash: String,
    pub importer_id: String,
    pub importer_version: u32,
    pub settings_hash: String,
    pub source_metadata: AssetImportSourceMetadata,
    pub license: AssetLicenseMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectAssetImportDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportDiagnostic {
    pub severity: ProjectAssetImportDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

impl ProjectAssetImportDiagnostic {
    fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ProjectAssetImportDiagnosticSeverity::Warning,
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
pub struct ProjectAssetImportCandidate {
    pub schema_version: String,
    pub import_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub source_byte_length: u64,
    pub record: AssetDatabaseRecord,
    pub meta: ProjectAssetMeta,
    pub database: AssetDatabaseDocument,
    pub graph: AssetGraphDocument,
    pub registry: AssetRegistryDocument,
    pub derived_before_digest: String,
    pub derived_candidate_digest: String,
    pub candidate_digest: String,
    pub conflict_policy: AssetImportConflictPolicy,
    pub revision: CandidateProjectRevision,
    pub candidate_store_root: String,
    pub diagnostics: Vec<ProjectAssetImportDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectAssetImportValidationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportValidationReport {
    pub schema_version: String,
    pub import_id: String,
    pub revision_id: String,
    pub candidate_digest: String,
    pub source_hash: String,
    pub derived_candidate_digest: String,
    pub status: ProjectAssetImportValidationStatus,
    pub texture_width: u32,
    pub texture_height: u32,
    pub validation_digest: String,
    pub diagnostics: Vec<ProjectAssetImportDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportApproval {
    pub schema_version: String,
    pub approved_by: String,
    pub candidate_digest: String,
    pub validation_digest: String,
    pub allow_replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportApplyRequest {
    pub candidate: ProjectAssetImportCandidate,
    pub validation_report: ProjectAssetImportValidationReport,
    pub approval: ProjectAssetImportApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportApplyReceipt {
    pub schema_version: String,
    pub import_id: String,
    pub revision: CandidateProjectRevision,
    pub source_hash: String,
    pub validation_digest: String,
    pub before_project_digest: String,
    pub applied_project_digest: String,
    pub derived_before_digest: String,
    pub derived_applied_digest: String,
    pub changed_paths: Vec<String>,
    pub rollback_record_path: String,
    pub rollback_record_digest: String,
    pub receipt_binding_digest: String,
    pub diagnostics: Vec<ProjectAssetImportDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportRollbackReceipt {
    pub schema_version: String,
    pub import_id: String,
    pub revision_id: String,
    pub restored_project_digest: String,
    pub replaced_project_digest: String,
    pub restored_derived_digest: String,
    pub replaced_derived_digest: String,
    pub changed_paths: Vec<String>,
    pub rollback_record_removed: bool,
    pub snapshot_files_removed: bool,
    pub diagnostics: Vec<ProjectAssetImportDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAssetImportError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub next_action: String,
}

impl ProjectAssetImportError {
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

impl std::fmt::Display for ProjectAssetImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectAssetImportError {}

pub struct ProjectAssetImport;

impl ProjectAssetImport {
    pub fn prepare(
        request: ProjectAssetImportPrepareRequest,
    ) -> Result<ProjectAssetImportCandidate, ProjectAssetImportError> {
        validate_token("import_id", &request.import_id)?;
        validate_token("revision_id", &request.revision_id)?;
        validate_asset_id(&request.asset_id)?;
        validate_display_name(&request.display_name)?;
        validate_source_metadata(&request.source_metadata)?;
        validate_license_metadata(&request.license)?;
        validate_texture_settings(&request.texture_settings)?;

        let target_directory = validate_target_directory(&request.target_directory)?;
        let source_target_path = format!("{target_directory}/{}.png", request.asset_id);
        let descriptor_path = format!("{target_directory}/{}.asset", request.asset_id);
        let meta_path = format!("{descriptor_path}.meta.json");
        let target_paths = [
            source_target_path.clone(),
            descriptor_path.clone(),
            meta_path.clone(),
        ];

        let scope = ProjectWriteScope::open(&request.project_root).map_err(project_write_error)?;
        let manifest = read_project_manifest(&scope)?;
        let (canonical_source, source_bytes, source_hash) =
            read_stable_source(&request.source_path)?;
        if canonical_source
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("png"))
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.unsupported_source_extension",
                "Formal Asset Import v1 only accepts PNG texture sources.",
                Some(&canonical_source),
                "Choose a PNG source or wait for the matching typed importer.",
            ));
        }

        let mut diagnostics = Vec::new();
        let mut database = load_database_from_scope(&scope)?
            .unwrap_or_else(|| AssetDatabaseDocument::empty(manifest.project_id.clone()));
        validate_database(&database, &manifest.project_id)?;
        collect_derived_diagnostics(&scope, &database, &mut diagnostics);
        reject_case_fold_collisions(&request.project_root, &target_paths)?;

        let existing = resolve_conflict(
            &scope,
            &database,
            &request.asset_id,
            &target_paths,
            &request.conflict_policy,
        )?;
        let asset_guid = existing
            .as_ref()
            .map(|record| record.asset_guid.clone())
            .unwrap_or_else(|| {
                asset_guid_for(&manifest.project_id, &request.import_id, &request.asset_id)
            });
        if database
            .assets
            .iter()
            .any(|record| record.asset_guid == asset_guid && record.asset_id != request.asset_id)
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.guid_conflict",
                "Generated or preserved asset GUID already belongs to another asset.",
                None,
                "Use a new import id or resolve the existing AssetDB conflict.",
            ));
        }

        let settings_hash = digest_serializable(&request.texture_settings, "texture settings")?;
        let record = AssetDatabaseRecord {
            asset_guid: asset_guid.clone(),
            asset_id: request.asset_id.clone(),
            display_name: request.display_name.clone(),
            asset_type: "texture".to_string(),
            descriptor_path: descriptor_path.clone(),
            source_path: source_target_path.clone(),
            meta_path: meta_path.clone(),
            source_hash: source_hash.clone(),
            source_byte_length: source_bytes.len() as u64,
            importer_id: TEXTURE_IMPORTER_ID.to_string(),
            importer_version: TEXTURE_IMPORTER_VERSION,
            settings_hash: settings_hash.clone(),
            state: AssetDatabaseRecordState::Current,
            source_metadata: request.source_metadata.clone(),
            license: request.license.clone(),
            direct_dependencies: Vec::new(),
        };
        let meta = ProjectAssetMeta {
            schema_version: PROJECT_ASSET_META_SCHEMA_VERSION.to_string(),
            asset_guid,
            asset_id: request.asset_id.clone(),
            asset_type: "texture".to_string(),
            descriptor_path: descriptor_path.clone(),
            source_path: source_target_path.clone(),
            source_hash: source_hash.clone(),
            importer_id: TEXTURE_IMPORTER_ID.to_string(),
            importer_version: TEXTURE_IMPORTER_VERSION,
            settings_hash,
            source_metadata: request.source_metadata.clone(),
            license: request.license.clone(),
        };
        database
            .assets
            .retain(|candidate| candidate.asset_id != request.asset_id);
        database.assets.push(record.clone());
        database
            .assets
            .sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        database.database_version = database.database_version.saturating_add(1);
        validate_database(&database, &manifest.project_id)?;
        let graph = build_graph(&database);
        let registry = build_registry(&database);
        let descriptor_bytes = json_bytes(&ImportedTextureDescriptor {
            schema_version: "texture-asset.v1".to_string(),
            asset_id: request.asset_id.clone(),
            asset_guid: record.asset_guid.clone(),
            display_name: request.display_name,
            source_image: source_target_path.clone(),
            importer: request.texture_settings,
        })?;
        let meta_bytes = json_bytes(&meta)?;
        let changes = vec![
            CandidateFileChange::CreateOrReplace {
                path: source_target_path,
                bytes: source_bytes,
            },
            CandidateFileChange::CreateOrReplace {
                path: descriptor_path,
                bytes: descriptor_bytes,
            },
            CandidateFileChange::CreateOrReplace {
                path: meta_path,
                bytes: meta_bytes,
            },
        ];
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
                ProjectAssetImportError::new(
                    "project_asset_import.candidate_store_missing",
                    "Staged candidate has no owning store directory.",
                    Some(Path::new(&revision.candidate_root)),
                    "Discard the invalid candidate and retry.",
                )
            })?
            .to_path_buf();
        let derived_before_digest = derived_state_digest(&scope)?;
        let derived_candidate_digest = derived_documents_digest(&database, &graph, &registry)?;
        if revision.changed_paths.is_empty() && derived_before_digest == derived_candidate_digest {
            let _ = CandidateProjectRevisionStore::discard(&revision, &candidate_store_root);
            return Err(ProjectAssetImportError::new(
                "project_asset_import.no_effect",
                "Asset import does not change project source or derived asset state.",
                Some(&canonical_source),
                "Change the source, metadata, settings, or target asset.",
            ));
        }
        if request.license.kind == AssetLicenseKind::Unknown {
            diagnostics.push(
                ProjectAssetImportDiagnostic::warning(
                    "project_asset_import.license_unknown",
                    "The asset was imported with an unknown license declaration.",
                )
                .with_path(canonical_source.display().to_string())
                .with_next_action("Resolve provenance before commercial distribution."),
            );
        }

        let mut candidate = ProjectAssetImportCandidate {
            schema_version: PROJECT_ASSET_IMPORT_CANDIDATE_SCHEMA_VERSION.to_string(),
            import_id: request.import_id,
            source_path: canonical_source.display().to_string(),
            source_hash,
            source_byte_length: record.source_byte_length,
            record,
            meta,
            database,
            graph,
            registry,
            derived_before_digest,
            derived_candidate_digest,
            candidate_digest: String::new(),
            conflict_policy: request.conflict_policy,
            revision,
            candidate_store_root: candidate_store_root.display().to_string(),
            diagnostics,
            next_actions: vec![
                "Validate the isolated asset candidate before requesting approval.".to_string(),
            ],
        };
        candidate.candidate_digest = candidate_digest(&candidate)?;
        validate_candidate_record(&candidate)?;
        Ok(candidate)
    }

    pub fn validate(
        candidate: &ProjectAssetImportCandidate,
    ) -> Result<ProjectAssetImportValidationReport, ProjectAssetImportError> {
        validate_candidate_record(candidate)?;
        let project_root = Path::new(&candidate.revision.project_root);
        let base = CandidateProjectRevisionStore::verify_base(&candidate.revision, project_root)
            .map_err(candidate_error)?;
        if base.status != CandidateBaseVerificationStatus::Matched {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.base_drifted",
                "Project source changed after the asset candidate was prepared.",
                Some(project_root),
                "Discard or rebase the candidate before validation.",
            ));
        }
        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        if derived_state_digest(&scope)? != candidate.derived_before_digest {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.derived_state_drifted",
                "AssetDB, AssetGraph, or AssetRegistry changed after candidate preparation.",
                Some(project_root),
                "Create a new asset import candidate from the current derived state.",
            ));
        }
        let (source_path, source_bytes, source_hash) =
            read_stable_source(Path::new(&candidate.source_path))?;
        if source_hash != candidate.source_hash
            || source_bytes.len() as u64 != candidate.source_byte_length
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.source_drifted",
                "External source bytes changed after candidate preparation.",
                Some(&source_path),
                "Prepare a new candidate from the current source file.",
            ));
        }
        let candidate_root = Path::new(&candidate.revision.candidate_root);
        let candidate_source = fs::read(candidate_root.join(&candidate.record.source_path))
            .map_err(|error| {
                ProjectAssetImportError::new(
                    "project_asset_import.candidate_source_read_failed",
                    format!("Candidate source cannot be read: {error}"),
                    Some(&candidate_root.join(&candidate.record.source_path)),
                    "Discard the damaged candidate and prepare it again.",
                )
            })?;
        if sha256_prefixed(&candidate_source) != candidate.source_hash {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.candidate_source_digest_mismatch",
                "Candidate source bytes no longer match the prepared source digest.",
                Some(&candidate_root.join(&candidate.record.source_path)),
                "Discard the modified candidate and prepare it again.",
            ));
        }
        validate_candidate_descriptor_and_meta(candidate)?;
        validate_database(&candidate.database, &candidate.database.project_id)?;
        if build_graph(&candidate.database) != candidate.graph
            || build_registry(&candidate.database) != candidate.registry
            || derived_documents_digest(&candidate.database, &candidate.graph, &candidate.registry)?
                != candidate.derived_candidate_digest
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.derived_candidate_invalid",
                "Candidate AssetDB, AssetGraph, and AssetRegistry are not deterministically bound.",
                Some(candidate_root),
                "Discard the invalid candidate and prepare it again.",
            ));
        }
        let (texture_width, texture_height) = validate_png(&candidate_source)?;
        let mut report = ProjectAssetImportValidationReport {
            schema_version: PROJECT_ASSET_IMPORT_VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
            import_id: candidate.import_id.clone(),
            revision_id: candidate.revision.revision_id.clone(),
            candidate_digest: candidate.candidate_digest.clone(),
            source_hash: candidate.source_hash.clone(),
            derived_candidate_digest: candidate.derived_candidate_digest.clone(),
            status: ProjectAssetImportValidationStatus::Passed,
            texture_width,
            texture_height,
            validation_digest: String::new(),
            diagnostics: candidate.diagnostics.clone(),
            next_actions: vec![
                "Review the import identity, provenance, license declaration, and validation result."
                    .to_string(),
                "Approve the exact candidate and validation digest before apply.".to_string(),
            ],
        };
        report.validation_digest = validation_report_digest(&report)?;
        Ok(report)
    }

    pub fn load_database(
        project_root: impl AsRef<Path>,
    ) -> Result<Option<AssetDatabaseDocument>, ProjectAssetImportError> {
        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        let manifest = read_project_manifest(&scope)?;
        let database = load_database_from_scope(&scope)?;
        if let Some(database) = &database {
            validate_database(database, &manifest.project_id)?;
        }
        Ok(database)
    }

    pub fn apply(
        request: ProjectAssetImportApplyRequest,
    ) -> Result<ProjectAssetImportApplyReceipt, ProjectAssetImportError> {
        Self::apply_internal(request, None)
    }

    fn apply_internal(
        request: ProjectAssetImportApplyRequest,
        fail_after_write: Option<usize>,
    ) -> Result<ProjectAssetImportApplyReceipt, ProjectAssetImportError> {
        validate_candidate_record(&request.candidate)?;
        validate_passed_report(&request.candidate, &request.validation_report)?;
        let project_root = Path::new(&request.candidate.revision.project_root);
        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        let import_lock = scope
            .acquire_lock(
                ASSET_IMPORT_LOCK_PATH,
                request.candidate.candidate_digest.as_bytes(),
            )
            .map_err(project_write_error)?;

        let current_report = Self::validate(&request.candidate)?;
        if current_report != request.validation_report {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.validation_replay_mismatch",
                "Validation evidence no longer matches the locked import candidate.",
                Some(project_root),
                "Validate and approve a new candidate from the current project state.",
            ));
        }
        validate_approval(
            &request.candidate,
            &request.validation_report,
            &request.approval,
        )?;

        let desired = desired_transaction_files(&request.candidate)?;
        let changed_paths = desired.keys().cloned().collect::<Vec<_>>();
        let rollback_record_path = rollback_record_path(&request.candidate);
        let mut rollback_record = snapshot_before_transaction(
            &scope,
            &request.candidate,
            &request.validation_report,
            &desired,
            &rollback_record_path,
        )?;
        rollback_record.record_digest = rollback_record_digest(&rollback_record)?;
        rollback_record.receipt_binding_digest = receipt_binding_digest(
            &request.candidate,
            &request.validation_report.validation_digest,
            &changed_paths,
            &rollback_record_path,
            &rollback_record.record_digest,
        )?;

        if let Err(error) = persist_rollback_record(&scope, &rollback_record_path, &rollback_record)
        {
            return cleanup_unapplied_snapshots(
                error,
                &scope,
                &rollback_record_path,
                &rollback_record.snapshots,
            );
        }

        if let Err(error) = apply_desired_files(&scope, &desired, fail_after_write) {
            return fail_apply_and_restore(
                error,
                &scope,
                &request.candidate.revision,
                &rollback_record,
                &rollback_record_path,
            );
        }
        if let Err(error) =
            verify_applied_transaction(&scope, &request.candidate, &rollback_record.snapshots)
        {
            return fail_apply_and_restore(
                error,
                &scope,
                &request.candidate.revision,
                &rollback_record,
                &rollback_record_path,
            );
        }

        let receipt = ProjectAssetImportApplyReceipt {
            schema_version: PROJECT_ASSET_IMPORT_APPLY_RECEIPT_SCHEMA_VERSION.to_string(),
            import_id: request.candidate.import_id.clone(),
            revision: request.candidate.revision.clone(),
            source_hash: request.candidate.source_hash.clone(),
            validation_digest: request.validation_report.validation_digest,
            before_project_digest: request.candidate.revision.base_project_digest.clone(),
            applied_project_digest: request.candidate.revision.candidate_project_digest.clone(),
            derived_before_digest: request.candidate.derived_before_digest,
            derived_applied_digest: request.candidate.derived_candidate_digest,
            changed_paths,
            rollback_record_path,
            rollback_record_digest: rollback_record.record_digest,
            receipt_binding_digest: rollback_record.receipt_binding_digest,
            diagnostics: request.candidate.diagnostics,
            next_actions: vec![
                "Keep the candidate and rollback snapshots until this import is accepted."
                    .to_string(),
            ],
        };
        import_lock.release().map_err(project_write_error)?;
        Ok(receipt)
    }

    pub fn rollback(
        receipt: &ProjectAssetImportApplyReceipt,
        project_root: &Path,
    ) -> Result<ProjectAssetImportRollbackReceipt, ProjectAssetImportError> {
        validate_apply_receipt(receipt)?;
        let scope = ProjectWriteScope::open(project_root).map_err(project_write_error)?;
        let import_lock = scope
            .acquire_lock(
                ASSET_IMPORT_LOCK_PATH,
                receipt.receipt_binding_digest.as_bytes(),
            )
            .map_err(project_write_error)?;
        verify_receipt_project_root(receipt, project_root)?;
        verify_current_applied_state(&scope, receipt)?;

        let record_bytes = scope
            .read(&receipt.rollback_record_path)
            .map_err(project_write_error)?;
        let record: AssetImportRollbackRecord =
            serde_json::from_slice(&record_bytes).map_err(|error| {
                ProjectAssetImportError::new(
                    "project_asset_import.rollback_record_invalid",
                    format!("Rollback record cannot be decoded: {error}"),
                    Some(Path::new(&receipt.rollback_record_path)),
                    "Preserve the rollback artifacts and recover with a trusted maintainer.",
                )
            })?;
        validate_rollback_record(&scope, receipt, &record)?;
        let applied_state = capture_current_files(&scope, &receipt.changed_paths)?;

        let rollback_result = restore_before_snapshots(&scope, &record.snapshots)
            .and_then(|_| verify_restored_before_state(&scope, receipt, &record.snapshots));
        if let Err(error) = rollback_result {
            return fail_rollback_and_restore_applied(error, &scope, receipt, &applied_state);
        }
        cleanup_rollback_artifacts(&scope, &receipt.rollback_record_path, &record.snapshots)?;
        import_lock.release().map_err(project_write_error)?;
        Ok(ProjectAssetImportRollbackReceipt {
            schema_version: PROJECT_ASSET_IMPORT_ROLLBACK_RECEIPT_SCHEMA_VERSION.to_string(),
            import_id: receipt.import_id.clone(),
            revision_id: receipt.revision.revision_id.clone(),
            restored_project_digest: receipt.before_project_digest.clone(),
            replaced_project_digest: receipt.applied_project_digest.clone(),
            restored_derived_digest: receipt.derived_before_digest.clone(),
            replaced_derived_digest: receipt.derived_applied_digest.clone(),
            changed_paths: receipt.changed_paths.clone(),
            rollback_record_removed: true,
            snapshot_files_removed: true,
            diagnostics: Vec::new(),
            next_actions: vec![
                "The project asset state is back at the pre-import revision.".to_string(),
            ],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetImportRollbackSnapshot {
    path: String,
    snapshot_path: Option<String>,
    before_digest: Option<String>,
    applied_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetImportRollbackRecord {
    schema_version: String,
    import_id: String,
    revision_id: String,
    validation_digest: String,
    source_hash: String,
    before_project_digest: String,
    applied_project_digest: String,
    derived_before_digest: String,
    derived_applied_digest: String,
    changed_paths: Vec<String>,
    record_digest: String,
    receipt_binding_digest: String,
    snapshots: Vec<AssetImportRollbackSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportedTextureDescriptor {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    #[serde(rename = "assetId")]
    asset_id: String,
    #[serde(rename = "assetGuid")]
    asset_guid: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "sourceImage")]
    source_image: String,
    importer: TextureImportSettings,
}

fn desired_transaction_files(
    candidate: &ProjectAssetImportCandidate,
) -> Result<BTreeMap<String, Option<Vec<u8>>>, ProjectAssetImportError> {
    let candidate_root = Path::new(&candidate.revision.candidate_root);
    let mut files = BTreeMap::new();
    for path in &candidate.revision.changed_paths {
        let source = candidate_root.join(path);
        let bytes = if source.exists() {
            Some(fs::read(&source).map_err(|error| {
                ProjectAssetImportError::new(
                    "project_asset_import.candidate_file_read_failed",
                    format!("Validated candidate file cannot be read: {error}"),
                    Some(&source),
                    "Reject the candidate and preserve the project base.",
                )
            })?)
        } else {
            None
        };
        files.insert(path.clone(), bytes);
    }
    for (path, bytes) in [
        (ASSET_DATABASE_PATH, json_bytes(&candidate.database)?),
        (ASSET_GRAPH_PATH, json_bytes(&candidate.graph)?),
        (ASSET_REGISTRY_PATH, json_bytes(&candidate.registry)?),
    ] {
        if files.insert(path.to_string(), Some(bytes)).is_some() {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.transaction_path_overlap",
                "Candidate source changes overlap AssetPipeline derived state.",
                Some(Path::new(path)),
                "Discard the invalid candidate and inspect its changed-path contract.",
            ));
        }
    }
    Ok(files)
}

fn rollback_transaction_id(candidate: &ProjectAssetImportCandidate) -> String {
    let payload = format!(
        "project-asset-import-transaction.v1\0{}\0{}",
        candidate.import_id, candidate.revision.revision_id
    );
    format!(
        "tx-{}",
        sha256_prefixed(payload.as_bytes()).trim_start_matches("sha256:")
    )
}

fn rollback_record_path(candidate: &ProjectAssetImportCandidate) -> String {
    format!(
        "Library/AssetPipeline/Rollback/{}/record.json",
        rollback_transaction_id(candidate)
    )
}

fn rollback_record_path_for(import_id: &str, revision_id: &str) -> String {
    let payload = format!("project-asset-import-transaction.v1\0{import_id}\0{revision_id}");
    format!(
        "Library/AssetPipeline/Rollback/tx-{}/record.json",
        sha256_prefixed(payload.as_bytes()).trim_start_matches("sha256:")
    )
}

fn snapshot_path(record_path: &str, index: usize) -> Result<String, ProjectAssetImportError> {
    let parent = Path::new(record_path).parent().ok_or_else(|| {
        ProjectAssetImportError::new(
            "project_asset_import.rollback_record_path_invalid",
            "Rollback record path has no project-relative parent.",
            Some(Path::new(record_path)),
            "Reject the invalid transaction path.",
        )
    })?;
    Ok(format!(
        "{}/snapshot-{index:03}.bin",
        parent.to_string_lossy().replace('\\', "/")
    ))
}

fn snapshot_before_transaction(
    scope: &ProjectWriteScope,
    candidate: &ProjectAssetImportCandidate,
    report: &ProjectAssetImportValidationReport,
    desired: &BTreeMap<String, Option<Vec<u8>>>,
    record_path: &str,
) -> Result<AssetImportRollbackRecord, ProjectAssetImportError> {
    if scope.try_exists(record_path).map_err(project_write_error)? {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_record_exists",
            "A rollback record already exists for this import transaction.",
            Some(Path::new(record_path)),
            "Resolve the previous import or rollback before retrying.",
        ));
    }

    let mut snapshots = Vec::with_capacity(desired.len());
    let mut before_payloads = Vec::new();
    for (index, (path, after_bytes)) in desired.iter().enumerate() {
        let binary_path = snapshot_path(record_path, index)?;
        if scope
            .try_exists(&binary_path)
            .map_err(project_write_error)?
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.rollback_snapshot_exists",
                "A rollback snapshot already exists for this import transaction.",
                Some(Path::new(&binary_path)),
                "Resolve the stale rollback artifacts before retrying.",
            ));
        }
        let before_bytes = if scope.try_exists(path).map_err(project_write_error)? {
            Some(scope.read(path).map_err(project_write_error)?)
        } else {
            None
        };
        let before_digest = before_bytes.as_deref().map(sha256_prefixed);
        let snapshot_path = before_bytes.as_ref().map(|_| binary_path.clone());
        let applied_digest = after_bytes.as_deref().map(sha256_prefixed);
        if let Some(bytes) = before_bytes {
            before_payloads.push((binary_path, bytes));
        }
        snapshots.push(AssetImportRollbackSnapshot {
            path: path.clone(),
            snapshot_path,
            before_digest,
            applied_digest,
        });
    }

    let mut written = Vec::new();
    for (path, bytes) in before_payloads {
        if let Err(error) = scope
            .write_atomic(&path, &bytes)
            .map_err(project_write_error)
        {
            let cleanup = cleanup_relative_files(scope, written.iter().rev());
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(ProjectAssetImportError::new(
                    "project_asset_import.snapshot_write_cleanup_failed",
                    format!(
                        "Rollback snapshot write failed ({error}); cleanup also failed: {cleanup_error}"
                    ),
                    Some(Path::new(&path)),
                    "Preserve the rollback directory and recover with a trusted maintainer.",
                )),
            };
        }
        written.push(path);
    }

    Ok(AssetImportRollbackRecord {
        schema_version: PROJECT_ASSET_IMPORT_ROLLBACK_RECORD_SCHEMA_VERSION.to_string(),
        import_id: candidate.import_id.clone(),
        revision_id: candidate.revision.revision_id.clone(),
        validation_digest: report.validation_digest.clone(),
        source_hash: candidate.source_hash.clone(),
        before_project_digest: candidate.revision.base_project_digest.clone(),
        applied_project_digest: candidate.revision.candidate_project_digest.clone(),
        derived_before_digest: candidate.derived_before_digest.clone(),
        derived_applied_digest: candidate.derived_candidate_digest.clone(),
        changed_paths: desired.keys().cloned().collect(),
        record_digest: String::new(),
        receipt_binding_digest: String::new(),
        snapshots,
    })
}

fn persist_rollback_record(
    scope: &ProjectWriteScope,
    record_path: &str,
    record: &AssetImportRollbackRecord,
) -> Result<(), ProjectAssetImportError> {
    let bytes = json_bytes(record)?;
    scope
        .write_atomic(record_path, &bytes)
        .map(drop)
        .map_err(project_write_error)
}

fn cleanup_unapplied_snapshots(
    cause: ProjectAssetImportError,
    scope: &ProjectWriteScope,
    record_path: &str,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<ProjectAssetImportApplyReceipt, ProjectAssetImportError> {
    match cleanup_rollback_artifacts(scope, record_path, snapshots) {
        Ok(()) => Err(cause),
        Err(cleanup_error) => Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_record_cleanup_failed",
            format!(
                "Rollback record persistence failed ({cause}); snapshot cleanup also failed: {cleanup_error}"
            ),
            Some(Path::new(record_path)),
            "Preserve the rollback directory and recover with a trusted maintainer.",
        )),
    }
}

fn apply_desired_files(
    scope: &ProjectWriteScope,
    desired: &BTreeMap<String, Option<Vec<u8>>>,
    fail_after_write: Option<usize>,
) -> Result<(), ProjectAssetImportError> {
    let mut written = 0_usize;
    for (path, bytes) in desired {
        if let Some(bytes) = bytes {
            scope
                .write_atomic(path, bytes)
                .map_err(project_write_error)?;
        } else {
            scope.remove_file(path).map_err(project_write_error)?;
        }
        written += 1;
        if fail_after_write == Some(written) {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.injected_apply_failure",
                "Asset import test fault interrupted the transaction.",
                Some(Path::new(path)),
                "Verify compensating restoration before retrying.",
            ));
        }
    }
    Ok(())
}

fn verify_applied_transaction(
    scope: &ProjectWriteScope,
    candidate: &ProjectAssetImportCandidate,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<(), ProjectAssetImportError> {
    let applied =
        CandidateProjectRevisionStore::verify_base(&candidate.revision, scope.display_root())
            .map_err(candidate_error)?;
    if applied.actual_digest != candidate.revision.candidate_project_digest
        || derived_state_digest(scope)? != candidate.derived_candidate_digest
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.apply_digest_mismatch",
            "Applied source or derived state does not match the validated import candidate.",
            Some(scope.display_root()),
            "Restore the before snapshots and inspect concurrent project writes.",
        ));
    }
    verify_snapshot_side(scope, snapshots, false)
}

fn fail_apply_and_restore(
    cause: ProjectAssetImportError,
    scope: &ProjectWriteScope,
    revision: &CandidateProjectRevision,
    record: &AssetImportRollbackRecord,
    record_path: &str,
) -> Result<ProjectAssetImportApplyReceipt, ProjectAssetImportError> {
    if let Err(restore_error) = restore_before_snapshots(scope, &record.snapshots) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.apply_rollback_failed",
            format!("Apply failed ({cause}); automatic restoration also failed: {restore_error}"),
            Some(scope.display_root()),
            "Preserve the rollback artifacts and recover with a trusted maintainer.",
        ));
    }
    let restored = CandidateProjectRevisionStore::verify_base(revision, scope.display_root())
        .map_err(candidate_error)?;
    if restored.status != CandidateBaseVerificationStatus::Matched
        || restored.actual_digest != record.before_project_digest
        || derived_state_digest(scope)? != record.derived_before_digest
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.apply_rollback_digest_mismatch",
            "Apply failed and restoration did not reproduce the recorded before state.",
            Some(scope.display_root()),
            "Preserve the rollback artifacts and recover with a trusted maintainer.",
        ));
    }
    cleanup_rollback_artifacts(scope, record_path, &record.snapshots)?;
    Err(ProjectAssetImportError::new(
        "project_asset_import.apply_failed_restored",
        format!("Asset import apply failed and the before state was restored: {cause}"),
        Some(scope.display_root()),
        "Repair the candidate and create a new validated import revision.",
    ))
}

fn restore_before_snapshots(
    scope: &ProjectWriteScope,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<(), ProjectAssetImportError> {
    let before = read_before_snapshots(scope, snapshots)?;
    for (snapshot, bytes) in snapshots.iter().zip(before).rev() {
        if let Some(bytes) = bytes {
            scope
                .write_atomic(&snapshot.path, &bytes)
                .map_err(project_write_error)?;
        } else {
            scope
                .remove_file(&snapshot.path)
                .map_err(project_write_error)?;
        }
    }
    Ok(())
}

fn read_before_snapshots(
    scope: &ProjectWriteScope,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<Vec<Option<Vec<u8>>>, ProjectAssetImportError> {
    snapshots
        .iter()
        .map(
            |snapshot| match (&snapshot.snapshot_path, &snapshot.before_digest) {
                (Some(path), Some(expected)) => {
                    let bytes = scope.read(path).map_err(project_write_error)?;
                    if sha256_prefixed(&bytes) != *expected {
                        return Err(ProjectAssetImportError::new(
                        "project_asset_import.rollback_snapshot_tampered",
                        "Rollback snapshot bytes do not match the recorded digest.",
                        Some(Path::new(path)),
                        "Preserve the rollback artifacts and recover with a trusted maintainer.",
                    ));
                    }
                    Ok(Some(bytes))
                }
                (None, None) => Ok(None),
                _ => Err(ProjectAssetImportError::new(
                    "project_asset_import.rollback_snapshot_binding_invalid",
                    "Rollback snapshot path and digest presence do not agree.",
                    Some(Path::new(&snapshot.path)),
                    "Reject the invalid rollback record.",
                )),
            },
        )
        .collect()
}

fn cleanup_rollback_artifacts(
    scope: &ProjectWriteScope,
    record_path: &str,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<(), ProjectAssetImportError> {
    let snapshot_paths = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.snapshot_path.as_ref());
    cleanup_relative_files(scope, snapshot_paths)?;
    scope
        .remove_file(record_path)
        .map(drop)
        .map_err(project_write_error)
}

fn cleanup_relative_files<'a>(
    scope: &ProjectWriteScope,
    paths: impl IntoIterator<Item = &'a String>,
) -> Result<(), ProjectAssetImportError> {
    for path in paths {
        scope.remove_file(path).map_err(project_write_error)?;
    }
    Ok(())
}

fn validate_passed_report(
    candidate: &ProjectAssetImportCandidate,
    report: &ProjectAssetImportValidationReport,
) -> Result<(), ProjectAssetImportError> {
    if report.schema_version != PROJECT_ASSET_IMPORT_VALIDATION_REPORT_SCHEMA_VERSION
        || report.status != ProjectAssetImportValidationStatus::Passed
        || report.import_id != candidate.import_id
        || report.revision_id != candidate.revision.revision_id
        || report.candidate_digest != candidate.candidate_digest
        || report.source_hash != candidate.source_hash
        || report.derived_candidate_digest != candidate.derived_candidate_digest
        || report.texture_width == 0
        || report.texture_height == 0
        || validation_report_digest(report).as_deref() != Ok(report.validation_digest.as_str())
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.validation_binding_mismatch",
            "Validation report does not bind the exact passed import candidate.",
            None,
            "Validate the exact candidate successfully before approval or apply.",
        ));
    }
    Ok(())
}

fn validate_approval(
    candidate: &ProjectAssetImportCandidate,
    report: &ProjectAssetImportValidationReport,
    approval: &ProjectAssetImportApproval,
) -> Result<(), ProjectAssetImportError> {
    let replacement_allowed = !matches!(
        candidate.conflict_policy,
        AssetImportConflictPolicy::ReplaceMatching { .. }
    ) || approval.allow_replace;
    if approval.schema_version != PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION
        || approval.candidate_digest != candidate.candidate_digest
        || approval.validation_digest != report.validation_digest
        || approval.approved_by.trim().is_empty()
        || approval.approved_by.len() > 128
        || !replacement_allowed
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.approval_binding_mismatch",
            "Approval does not authorize the exact validated import candidate.",
            None,
            "Request explicit approval for this candidate, validation digest, and replacement mode.",
        ));
    }
    Ok(())
}

fn rollback_record_digest(
    record: &AssetImportRollbackRecord,
) -> Result<String, ProjectAssetImportError> {
    let mut normalized = record.clone();
    normalized.record_digest.clear();
    normalized.receipt_binding_digest.clear();
    digest_serializable(&normalized, "asset import rollback record")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptBindingFields<'a> {
    import_id: &'a str,
    revision_id: &'a str,
    source_hash: &'a str,
    before_project_digest: &'a str,
    applied_project_digest: &'a str,
    derived_before_digest: &'a str,
    derived_applied_digest: &'a str,
    validation_digest: &'a str,
    changed_paths: &'a [String],
    rollback_record_path: &'a str,
    rollback_record_digest: &'a str,
}

fn receipt_binding_digest_fields(
    fields: &ReceiptBindingFields<'_>,
) -> Result<String, ProjectAssetImportError> {
    digest_serializable(fields, "asset import apply receipt binding")
}

fn receipt_binding_digest(
    candidate: &ProjectAssetImportCandidate,
    validation_digest: &str,
    changed_paths: &[String],
    record_path: &str,
    record_digest: &str,
) -> Result<String, ProjectAssetImportError> {
    receipt_binding_digest_fields(&ReceiptBindingFields {
        import_id: &candidate.import_id,
        revision_id: &candidate.revision.revision_id,
        source_hash: &candidate.source_hash,
        before_project_digest: &candidate.revision.base_project_digest,
        applied_project_digest: &candidate.revision.candidate_project_digest,
        derived_before_digest: &candidate.derived_before_digest,
        derived_applied_digest: &candidate.derived_candidate_digest,
        validation_digest,
        changed_paths,
        rollback_record_path: record_path,
        rollback_record_digest: record_digest,
    })
}

fn validate_apply_receipt(
    receipt: &ProjectAssetImportApplyReceipt,
) -> Result<(), ProjectAssetImportError> {
    validate_token("import_id", &receipt.import_id)?;
    validate_digest_format(&receipt.source_hash, "source hash")?;
    validate_digest_format(&receipt.validation_digest, "validation digest")?;
    validate_digest_format(&receipt.before_project_digest, "before project digest")?;
    validate_digest_format(&receipt.applied_project_digest, "applied project digest")?;
    validate_digest_format(&receipt.derived_before_digest, "derived before digest")?;
    validate_digest_format(&receipt.derived_applied_digest, "derived applied digest")?;
    validate_digest_format(&receipt.rollback_record_digest, "rollback record digest")?;
    validate_digest_format(&receipt.receipt_binding_digest, "receipt binding digest")?;
    let expected_record_path =
        rollback_record_path_for(&receipt.import_id, &receipt.revision.revision_id);
    let mut canonical_paths = BTreeSet::new();
    for path in &receipt.changed_paths {
        let relative = ProjectRelativePath::parse(path).map_err(project_write_error)?;
        if relative.as_str() != path || !canonical_paths.insert(path) {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.apply_receipt_paths_invalid",
                "Apply receipt changed paths must be canonical, unique, and sorted.",
                Some(Path::new(path)),
                "Reject the invalid apply receipt.",
            ));
        }
    }
    let sorted_paths = canonical_paths.into_iter().cloned().collect::<Vec<_>>();
    let binding = receipt_binding_digest_fields(&ReceiptBindingFields {
        import_id: &receipt.import_id,
        revision_id: &receipt.revision.revision_id,
        source_hash: &receipt.source_hash,
        before_project_digest: &receipt.before_project_digest,
        applied_project_digest: &receipt.applied_project_digest,
        derived_before_digest: &receipt.derived_before_digest,
        derived_applied_digest: &receipt.derived_applied_digest,
        validation_digest: &receipt.validation_digest,
        changed_paths: &receipt.changed_paths,
        rollback_record_path: &receipt.rollback_record_path,
        rollback_record_digest: &receipt.rollback_record_digest,
    })?;
    let mut expected_paths = receipt.revision.changed_paths.clone();
    expected_paths.extend([
        ASSET_DATABASE_PATH.to_string(),
        ASSET_GRAPH_PATH.to_string(),
        ASSET_REGISTRY_PATH.to_string(),
    ]);
    expected_paths.sort();
    if receipt.schema_version != PROJECT_ASSET_IMPORT_APPLY_RECEIPT_SCHEMA_VERSION
        || receipt.before_project_digest != receipt.revision.base_project_digest
        || receipt.applied_project_digest != receipt.revision.candidate_project_digest
        || receipt.changed_paths != sorted_paths
        || receipt.changed_paths != expected_paths
        || receipt.rollback_record_path != expected_record_path
        || receipt.receipt_binding_digest != binding
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.apply_receipt_binding_mismatch",
            "Apply receipt does not bind the exact import transaction.",
            Some(Path::new(&receipt.rollback_record_path)),
            "Reject the invalid apply receipt.",
        ));
    }
    Ok(())
}

fn validate_digest_format(value: &str, role: &str) -> Result<(), ProjectAssetImportError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(ProjectAssetImportError::new(
            "project_asset_import.digest_invalid",
            format!("Recorded {role} is not a canonical SHA-256 digest."),
            None,
            "Reject the invalid import artifact.",
        ))
    }
}

fn verify_receipt_project_root(
    receipt: &ProjectAssetImportApplyReceipt,
    project_root: &Path,
) -> Result<(), ProjectAssetImportError> {
    let actual = fs::canonicalize(project_root).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.project_root_unavailable",
            format!("Project root cannot be resolved: {error}"),
            Some(project_root),
            "Open the original project root before rollback.",
        )
    })?;
    let expected = fs::canonicalize(&receipt.revision.project_root).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.project_root_unavailable",
            format!("Receipt project root cannot be resolved: {error}"),
            Some(Path::new(&receipt.revision.project_root)),
            "Restore the original project root before rollback.",
        )
    })?;
    if !import_paths_equal(&actual, &expected) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.project_root_mismatch",
            "Rollback project root does not match the applied import receipt.",
            Some(&actual),
            "Rollback only against the original project root.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn import_paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn import_paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

fn verify_current_applied_state(
    scope: &ProjectWriteScope,
    receipt: &ProjectAssetImportApplyReceipt,
) -> Result<(), ProjectAssetImportError> {
    let current =
        CandidateProjectRevisionStore::verify_base(&receipt.revision, scope.display_root())
            .map_err(candidate_error)?;
    if current.actual_digest != receipt.applied_project_digest
        || derived_state_digest(scope)? != receipt.derived_applied_digest
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_project_drifted",
            "Project source or AssetPipeline state changed after import apply.",
            Some(scope.display_root()),
            "Review current changes and perform an explicit merge or recovery.",
        ));
    }
    Ok(())
}

fn validate_rollback_record(
    scope: &ProjectWriteScope,
    receipt: &ProjectAssetImportApplyReceipt,
    record: &AssetImportRollbackRecord,
) -> Result<(), ProjectAssetImportError> {
    let digest_matches =
        rollback_record_digest(record).as_deref() == Ok(receipt.rollback_record_digest.as_str());
    let paths_match = record.snapshots.len() == receipt.changed_paths.len()
        && record
            .snapshots
            .iter()
            .map(|snapshot| &snapshot.path)
            .eq(receipt.changed_paths.iter());
    if record.schema_version != PROJECT_ASSET_IMPORT_ROLLBACK_RECORD_SCHEMA_VERSION
        || record.import_id != receipt.import_id
        || record.revision_id != receipt.revision.revision_id
        || record.validation_digest != receipt.validation_digest
        || record.source_hash != receipt.source_hash
        || record.before_project_digest != receipt.before_project_digest
        || record.applied_project_digest != receipt.applied_project_digest
        || record.derived_before_digest != receipt.derived_before_digest
        || record.derived_applied_digest != receipt.derived_applied_digest
        || record.changed_paths != receipt.changed_paths
        || record.record_digest != receipt.rollback_record_digest
        || record.receipt_binding_digest != receipt.receipt_binding_digest
        || !digest_matches
        || !paths_match
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_record_binding_mismatch",
            "Rollback record does not bind the exact applied import receipt.",
            Some(Path::new(&receipt.rollback_record_path)),
            "Preserve the record and recover with a trusted maintainer.",
        ));
    }
    for (index, snapshot) in record.snapshots.iter().enumerate() {
        let expected_snapshot_path = snapshot_path(&receipt.rollback_record_path, index)?;
        match (&snapshot.snapshot_path, &snapshot.before_digest) {
            (Some(path), Some(digest))
                if path == &expected_snapshot_path
                    && validate_digest_format(digest, "snapshot digest").is_ok() => {}
            (None, None) => {}
            _ => {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.rollback_snapshot_binding_invalid",
                    "Rollback snapshot metadata is not canonical for the transaction.",
                    Some(Path::new(&snapshot.path)),
                    "Reject the invalid rollback record.",
                ));
            }
        }
        if let Some(digest) = &snapshot.applied_digest {
            validate_digest_format(digest, "applied file digest")?;
        }
    }
    read_before_snapshots(scope, &record.snapshots)?;
    verify_snapshot_side(scope, &record.snapshots, false)
}

fn verify_snapshot_side(
    scope: &ProjectWriteScope,
    snapshots: &[AssetImportRollbackSnapshot],
    before: bool,
) -> Result<(), ProjectAssetImportError> {
    for snapshot in snapshots {
        let expected = if before {
            snapshot.before_digest.as_ref()
        } else {
            snapshot.applied_digest.as_ref()
        };
        let exists = scope
            .try_exists(&snapshot.path)
            .map_err(project_write_error)?;
        match (exists, expected) {
            (true, Some(expected)) => {
                let bytes = scope.read(&snapshot.path).map_err(project_write_error)?;
                if sha256_prefixed(&bytes) != *expected {
                    return Err(ProjectAssetImportError::new(
                        "project_asset_import.transaction_file_digest_mismatch",
                        "Transaction file bytes do not match the recorded state.",
                        Some(Path::new(&snapshot.path)),
                        "Preserve rollback artifacts and inspect concurrent writes.",
                    ));
                }
            }
            (false, None) => {}
            _ => {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.transaction_file_presence_mismatch",
                    "Transaction file presence does not match the recorded state.",
                    Some(Path::new(&snapshot.path)),
                    "Preserve rollback artifacts and inspect concurrent writes.",
                ));
            }
        }
    }
    Ok(())
}

fn capture_current_files(
    scope: &ProjectWriteScope,
    paths: &[String],
) -> Result<Vec<CapturedFileState>, ProjectAssetImportError> {
    paths
        .iter()
        .map(|path| {
            let bytes = if scope.try_exists(path).map_err(project_write_error)? {
                Some(scope.read(path).map_err(project_write_error)?)
            } else {
                None
            };
            Ok((path.clone(), bytes))
        })
        .collect()
}

fn restore_file_state(
    scope: &ProjectWriteScope,
    files: &[CapturedFileState],
) -> Result<(), ProjectAssetImportError> {
    for (path, bytes) in files.iter().rev() {
        if let Some(bytes) = bytes {
            scope
                .write_atomic(path, bytes)
                .map_err(project_write_error)?;
        } else {
            scope.remove_file(path).map_err(project_write_error)?;
        }
    }
    Ok(())
}

fn verify_restored_before_state(
    scope: &ProjectWriteScope,
    receipt: &ProjectAssetImportApplyReceipt,
    snapshots: &[AssetImportRollbackSnapshot],
) -> Result<(), ProjectAssetImportError> {
    let restored =
        CandidateProjectRevisionStore::verify_base(&receipt.revision, scope.display_root())
            .map_err(candidate_error)?;
    if restored.status != CandidateBaseVerificationStatus::Matched
        || restored.actual_digest != receipt.before_project_digest
        || derived_state_digest(scope)? != receipt.derived_before_digest
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_digest_mismatch",
            "Rollback did not restore the recorded before state.",
            Some(scope.display_root()),
            "Preserve rollback artifacts and recover with a trusted maintainer.",
        ));
    }
    verify_snapshot_side(scope, snapshots, true)
}

fn fail_rollback_and_restore_applied(
    cause: ProjectAssetImportError,
    scope: &ProjectWriteScope,
    receipt: &ProjectAssetImportApplyReceipt,
    applied_state: &[CapturedFileState],
) -> Result<ProjectAssetImportRollbackReceipt, ProjectAssetImportError> {
    if let Err(restore_error) = restore_file_state(scope, applied_state) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_compensation_failed",
            format!(
                "Rollback failed ({cause}); restoring the applied state also failed: {restore_error}"
            ),
            Some(scope.display_root()),
            "Preserve rollback artifacts and recover with a trusted maintainer.",
        ));
    }
    if let Err(verify_error) = verify_current_applied_state(scope, receipt) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.rollback_compensation_digest_mismatch",
            format!(
                "Rollback failed ({cause}); applied-state compensation could not be verified: {verify_error}"
            ),
            Some(scope.display_root()),
            "Preserve rollback artifacts and recover with a trusted maintainer.",
        ));
    }
    Err(ProjectAssetImportError::new(
        "project_asset_import.rollback_failed_applied_restored",
        format!("Rollback failed and the applied state was restored: {cause}"),
        Some(scope.display_root()),
        "Repair the rollback artifacts before retrying.",
    ))
}

fn validate_token(field: &str, value: &str) -> Result<(), ProjectAssetImportError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ProjectAssetImportError::new(
            format!("project_asset_import.{field}_invalid"),
            format!("{field} must use 1..128 ASCII letters, numbers, '-' or '_'."),
            None,
            format!("Provide a stable path-safe {field}."),
        ));
    }
    Ok(())
}

fn validate_asset_id(value: &str) -> Result<(), ProjectAssetImportError> {
    validate_token("asset_id", value)?;
    if is_windows_reserved_name(value) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.asset_id_reserved",
            "Asset id is a reserved Windows file name.",
            None,
            "Choose a portable asset id.",
        ));
    }
    Ok(())
}

fn validate_display_name(value: &str) -> Result<(), ProjectAssetImportError> {
    if value.trim().is_empty() || value.len() > 256 {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.display_name_invalid",
            "Display name must contain 1..256 characters.",
            None,
            "Provide a concise user-facing asset name.",
        ));
    }
    Ok(())
}

fn validate_source_metadata(
    metadata: &AssetImportSourceMetadata,
) -> Result<(), ProjectAssetImportError> {
    validate_optional_text("source creator", metadata.creator.as_deref(), 512)?;
    validate_optional_text("source note", metadata.note.as_deref(), 2048)?;
    validate_optional_uri("source URI", metadata.source_uri.as_deref())
}

fn validate_license_metadata(
    metadata: &AssetLicenseMetadata,
) -> Result<(), ProjectAssetImportError> {
    validate_optional_text("license identifier", metadata.identifier.as_deref(), 256)?;
    validate_optional_text("license attribution", metadata.attribution.as_deref(), 2048)?;
    validate_optional_text("license note", metadata.note.as_deref(), 2048)?;
    validate_optional_uri("license URI", metadata.license_uri.as_deref())?;
    match metadata.kind {
        AssetLicenseKind::ProjectOwned => {}
        AssetLicenseKind::ThirdParty
            if metadata
                .identifier
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()) => {}
        AssetLicenseKind::Unknown
            if metadata
                .note
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()) => {}
        AssetLicenseKind::ThirdParty => {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.third_party_license_identifier_required",
                "Third-party assets require a license identifier.",
                None,
                "Record the SPDX id or a stable license name.",
            ));
        }
        AssetLicenseKind::Unknown => {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.unknown_license_note_required",
                "Unknown license declarations require an explanatory note.",
                None,
                "Explain why provenance is unresolved.",
            ));
        }
    }
    Ok(())
}

fn validate_optional_text(
    field: &str,
    value: Option<&str>,
    max_length: usize,
) -> Result<(), ProjectAssetImportError> {
    if value.is_some_and(|value| value.trim().is_empty() || value.len() > max_length) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.metadata_text_invalid",
            format!("{field} must be non-empty and at most {max_length} characters when present."),
            None,
            "Remove the field or provide bounded metadata.",
        ));
    }
    Ok(())
}

fn validate_optional_uri(field: &str, value: Option<&str>) -> Result<(), ProjectAssetImportError> {
    if let Some(value) = value {
        Url::parse(value).map_err(|error| {
            ProjectAssetImportError::new(
                "project_asset_import.metadata_uri_invalid",
                format!("{field} is invalid: {error}"),
                None,
                "Provide an absolute URI with a scheme.",
            )
        })?;
    }
    Ok(())
}

fn validate_texture_settings(
    settings: &TextureImportSettings,
) -> Result<(), ProjectAssetImportError> {
    if !matches!(settings.color_space.as_str(), "srgb" | "linear") {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.color_space_invalid",
            "Texture color space must be 'srgb' or 'linear'.",
            None,
            "Choose the color space required by the material contract.",
        ));
    }
    if !matches!(
        settings.sampler.as_str(),
        "linearClamp" | "linearRepeat" | "nearestClamp" | "nearestRepeat"
    ) {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.sampler_invalid",
            "Texture sampler is not supported by the v1 importer.",
            None,
            "Use linearClamp, linearRepeat, nearestClamp, or nearestRepeat.",
        ));
    }
    Ok(())
}

fn validate_target_directory(value: &str) -> Result<String, ProjectAssetImportError> {
    let relative = ProjectRelativePath::parse(value).map_err(project_write_error)?;
    if relative.as_str() != "Assets" && !relative.as_str().starts_with("Assets/") {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.target_outside_assets",
            "Asset import target directory must be Assets or one of its descendants.",
            Some(relative.as_path()),
            "Choose a project-owned Assets directory.",
        ));
    }
    for component in relative.as_path().components() {
        if let Component::Normal(value) = component {
            let value = value.to_string_lossy();
            if is_windows_reserved_name(&value) {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.target_reserved",
                    "Asset target contains a reserved Windows path component.",
                    Some(relative.as_path()),
                    "Choose a portable target folder.",
                ));
            }
        }
    }
    Ok(relative.as_str().to_string())
}

fn is_windows_reserved_name(value: &str) -> bool {
    let value = value.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    matches!(value.as_str(), "con" | "prn" | "aux" | "nul")
        || (value.len() == 4
            && (value.starts_with("com") || value.starts_with("lpt"))
            && value.as_bytes()[3].is_ascii_digit()
            && value.as_bytes()[3] != b'0')
}

fn read_project_manifest(
    scope: &ProjectWriteScope,
) -> Result<ProjectManifest, ProjectAssetImportError> {
    let bytes = scope
        .read("project.aife.json")
        .map_err(project_write_error)?;
    let manifest: ProjectManifest = serde_json::from_slice(&bytes).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.project_manifest_invalid",
            format!("Project manifest cannot be parsed: {error}"),
            Some(Path::new("project.aife.json")),
            "Repair the project before importing assets.",
        )
    })?;
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.project_manifest_schema_unsupported",
            "Asset import requires the current project manifest schema.",
            Some(Path::new("project.aife.json")),
            "Migrate the project explicitly before importing assets.",
        ));
    }
    Ok(manifest)
}

fn read_stable_source(
    source_path: &Path,
) -> Result<(PathBuf, Vec<u8>, String), ProjectAssetImportError> {
    let canonical = fs::canonicalize(source_path).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.source_unavailable",
            format!("Source file cannot be resolved: {error}"),
            Some(source_path),
            "Select an existing readable source file.",
        )
    })?;
    let before = fs::metadata(&canonical).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.source_metadata_failed",
            format!("Source metadata cannot be read: {error}"),
            Some(&canonical),
            "Restore the source file and retry.",
        )
    })?;
    if !before.is_file() {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.source_not_regular_file",
            "Asset import source must resolve to a regular file.",
            Some(&canonical),
            "Choose a regular PNG file.",
        ));
    }
    if before.len() == 0 || before.len() > MAX_SOURCE_BYTES {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.source_size_invalid",
            format!(
                "Source size must be between 1 and {MAX_SOURCE_BYTES} bytes; got {}.",
                before.len()
            ),
            Some(&canonical),
            "Choose a bounded non-empty source file.",
        ));
    }
    let bytes = fs::read(&canonical).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.source_read_failed",
            format!("Source bytes cannot be read: {error}"),
            Some(&canonical),
            "Restore read access and retry.",
        )
    })?;
    let after = fs::metadata(&canonical).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.source_metadata_failed",
            format!("Source metadata cannot be re-read: {error}"),
            Some(&canonical),
            "Retry after the source file is stable.",
        )
    })?;
    if before.len() != after.len() || after.len() != bytes.len() as u64 {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.source_changed_during_read",
            "Source file changed while it was being read.",
            Some(&canonical),
            "Wait for the producer to finish and prepare a new candidate.",
        ));
    }
    let hash = sha256_prefixed(&bytes);
    Ok((canonical, bytes, hash))
}

fn load_database_from_scope(
    scope: &ProjectWriteScope,
) -> Result<Option<AssetDatabaseDocument>, ProjectAssetImportError> {
    if !scope
        .try_exists(ASSET_DATABASE_PATH)
        .map_err(project_write_error)?
    {
        return Ok(None);
    }
    let bytes = scope
        .read(ASSET_DATABASE_PATH)
        .map_err(project_write_error)?;
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.database_invalid",
            format!("AssetDB cannot be parsed: {error}"),
            Some(Path::new(ASSET_DATABASE_PATH)),
            "Restore AssetDB or rebuild it from trusted metadata before importing.",
        )
    })
}

fn validate_database(
    database: &AssetDatabaseDocument,
    expected_project_id: &str,
) -> Result<(), ProjectAssetImportError> {
    if database.schema_version != PROJECT_ASSET_DATABASE_SCHEMA_VERSION
        || database.project_id != expected_project_id
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.database_identity_invalid",
            "AssetDB schema or project identity does not match the active project.",
            Some(Path::new(ASSET_DATABASE_PATH)),
            "Restore the AssetDB for this project.",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut guids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for record in &database.assets {
        validate_asset_id(&record.asset_id)?;
        let guid_valid = record.asset_guid.len() == 70
            && record.asset_guid.starts_with("asset-")
            && record.asset_guid[6..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
        if !guid_valid
            || !ids.insert(record.asset_id.to_ascii_lowercase())
            || !guids.insert(record.asset_guid.to_ascii_lowercase())
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.database_identity_conflict",
                "AssetDB contains a duplicate or empty asset identity.",
                Some(Path::new(ASSET_DATABASE_PATH)),
                "Resolve duplicate asset ids and GUIDs before importing.",
            ));
        }
        for path in [
            &record.descriptor_path,
            &record.source_path,
            &record.meta_path,
        ] {
            let relative = ProjectRelativePath::parse(path).map_err(project_write_error)?;
            if !relative.as_str().starts_with("Assets/")
                || !paths.insert(relative.as_str().to_ascii_lowercase())
            {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.database_path_conflict",
                    "AssetDB contains an invalid or duplicate project asset path.",
                    Some(Path::new(path)),
                    "Resolve AssetDB path conflicts before importing.",
                ));
            }
        }
        let supported_typed_importer = (record.asset_type == "texture"
            && record.importer_id == TEXTURE_IMPORTER_ID
            && record.importer_version == TEXTURE_IMPORTER_VERSION)
            || (record.asset_type == FONT_SOURCE_ASSET_TYPE
                && record.importer_id == FONT_SOURCE_IMPORTER_ID
                && record.importer_version == FONT_SOURCE_IMPORTER_VERSION);
        if !supported_typed_importer
            || record.state != AssetDatabaseRecordState::Current
            || record.source_byte_length == 0
            || record.source_byte_length > MAX_SOURCE_BYTES
            || validate_digest_format(&record.source_hash, "AssetDB source hash").is_err()
            || validate_digest_format(&record.settings_hash, "AssetDB settings hash").is_err()
        {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.database_record_unsupported",
                "AssetDB contains a record unsupported by Formal Asset Import v1.",
                Some(Path::new(ASSET_DATABASE_PATH)),
                "Use the typed importer that owns this record schema.",
            ));
        }
        validate_source_metadata(&record.source_metadata)?;
        validate_license_metadata(&record.license)?;
    }
    Ok(())
}

fn collect_derived_diagnostics(
    scope: &ProjectWriteScope,
    database: &AssetDatabaseDocument,
    diagnostics: &mut Vec<ProjectAssetImportDiagnostic>,
) {
    let expected_graph = build_graph(database);
    let expected_registry = build_registry(database);
    if optional_document_matches(scope, ASSET_GRAPH_PATH, &expected_graph).is_none_or(|ok| !ok) {
        diagnostics.push(
            ProjectAssetImportDiagnostic::warning(
                "project_asset_import.graph_rebuild_required",
                "AssetGraph is missing, invalid, or stale and will be rebuilt from AssetDB.",
            )
            .with_path(ASSET_GRAPH_PATH),
        );
    }
    if optional_document_matches(scope, ASSET_REGISTRY_PATH, &expected_registry)
        .is_none_or(|ok| !ok)
    {
        diagnostics.push(
            ProjectAssetImportDiagnostic::warning(
                "project_asset_import.registry_rebuild_required",
                "AssetRegistry is missing, invalid, or stale and will be rebuilt from AssetDB.",
            )
            .with_path(ASSET_REGISTRY_PATH),
        );
    }
}

fn optional_document_matches<T>(scope: &ProjectWriteScope, path: &str, expected: &T) -> Option<bool>
where
    T: for<'de> Deserialize<'de> + PartialEq,
{
    if !scope.try_exists(path).ok()? {
        return None;
    }
    let bytes = scope.read(path).ok()?;
    Some(serde_json::from_slice::<T>(&bytes).is_ok_and(|value| value == *expected))
}

fn build_graph(database: &AssetDatabaseDocument) -> AssetGraphDocument {
    let mut nodes = database
        .assets
        .iter()
        .map(|record| AssetGraphNode {
            asset_guid: record.asset_guid.clone(),
            asset_id: record.asset_id.clone(),
            direct_dependencies: record.direct_dependencies.clone(),
            source_paths: vec![
                record.descriptor_path.clone(),
                record.source_path.clone(),
                record.meta_path.clone(),
            ],
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.asset_guid.cmp(&right.asset_guid));
    AssetGraphDocument {
        schema_version: PROJECT_ASSET_GRAPH_SCHEMA_VERSION.to_string(),
        built_from_database_version: database.database_version,
        nodes,
    }
}

fn build_registry(database: &AssetDatabaseDocument) -> AssetRegistryDocument {
    let mut entries = database
        .assets
        .iter()
        .map(|record| AssetRegistryEntry {
            asset_guid: record.asset_guid.clone(),
            asset_id: record.asset_id.clone(),
            asset_type: record.asset_type.clone(),
            descriptor_path: record.descriptor_path.clone(),
            source_path: record.source_path.clone(),
            meta_path: record.meta_path.clone(),
            source_hash: record.source_hash.clone(),
            importer_id: record.importer_id.clone(),
            importer_version: record.importer_version,
            direct_dependencies: record.direct_dependencies.clone(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.asset_guid.cmp(&right.asset_guid));
    AssetRegistryDocument {
        schema_version: PROJECT_ASSET_REGISTRY_SCHEMA_VERSION.to_string(),
        registry_version: database.database_version,
        built_from_database_version: database.database_version,
        entries,
    }
}

fn resolve_conflict(
    scope: &ProjectWriteScope,
    database: &AssetDatabaseDocument,
    asset_id: &str,
    target_paths: &[String; 3],
    policy: &AssetImportConflictPolicy,
) -> Result<Option<AssetDatabaseRecord>, ProjectAssetImportError> {
    let existing = database.asset_by_id(asset_id).cloned();
    match policy {
        AssetImportConflictPolicy::RejectExisting => {
            let mut target_exists = false;
            for path in target_paths {
                target_exists |= scope.try_exists(path).map_err(project_write_error)?;
            }
            if existing.is_some() || target_exists {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.target_conflict",
                    "Asset id or target path already exists.",
                    Some(Path::new(&target_paths[0])),
                    "Choose a new asset id or use ReplaceMatching with exact expectations.",
                ));
            }
            Ok(None)
        }
        AssetImportConflictPolicy::ReplaceMatching {
            expected_asset_guid,
            expected_source_hash,
        } => {
            let record = existing.ok_or_else(|| {
                ProjectAssetImportError::new(
                    "project_asset_import.replace_target_missing",
                    "ReplaceMatching requires an existing AssetDB record.",
                    Some(Path::new(ASSET_DATABASE_PATH)),
                    "Refresh AssetDB or use RejectExisting for a new asset.",
                )
            })?;
            if &record.asset_guid != expected_asset_guid
                || &record.source_hash != expected_source_hash
                || record.source_path != target_paths[0]
                || record.descriptor_path != target_paths[1]
                || record.meta_path != target_paths[2]
            {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.replace_expectation_mismatch",
                    "ReplaceMatching expectations do not match the current asset identity.",
                    Some(Path::new(ASSET_DATABASE_PATH)),
                    "Review the current record and prepare a new replacement candidate.",
                ));
            }
            for path in target_paths {
                if !scope.try_exists(path).map_err(project_write_error)? {
                    return Err(ProjectAssetImportError::new(
                        "project_asset_import.replace_file_missing",
                        "Existing AssetDB record is missing a required project file.",
                        Some(Path::new(path)),
                        "Repair the asset record before reimporting.",
                    ));
                }
            }
            Ok(Some(record))
        }
    }
}

fn reject_case_fold_collisions(
    project_root: &Path,
    target_paths: &[String; 3],
) -> Result<(), ProjectAssetImportError> {
    let assets_root = project_root.join("Assets");
    if !assets_root.exists() {
        return Ok(());
    }
    let mut existing = BTreeMap::<String, String>::new();
    collect_asset_paths(project_root, &assets_root, &mut existing)?;
    for target in target_paths {
        if let Some(actual) = existing.get(&target.to_ascii_lowercase()) {
            if actual != target {
                return Err(ProjectAssetImportError::new(
                    "project_asset_import.case_fold_collision",
                    format!("Target path {target} collides with existing path {actual}."),
                    Some(Path::new(actual)),
                    "Choose a path that is unique under case-insensitive comparison.",
                ));
            }
        }
    }
    Ok(())
}

fn collect_asset_paths(
    project_root: &Path,
    directory: &Path,
    paths: &mut BTreeMap<String, String>,
) -> Result<(), ProjectAssetImportError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.asset_directory_read_failed",
            format!("Assets directory cannot be read: {error}"),
            Some(directory),
            "Restore readable project asset directories.",
        )
    })?;
    for entry in entries {
        let path = entry
            .map_err(|error| {
                ProjectAssetImportError::new(
                    "project_asset_import.asset_directory_entry_failed",
                    format!("Assets directory entry cannot be read: {error}"),
                    Some(directory),
                    "Restore readable project asset directories.",
                )
            })?
            .path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ProjectAssetImportError::new(
                "project_asset_import.asset_metadata_failed",
                format!("Asset path metadata cannot be read: {error}"),
                Some(&path),
                "Restore a regular project asset tree.",
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ProjectAssetImportError::new(
                "project_asset_import.asset_link_rejected",
                "Asset import does not traverse symbolic links or junctions.",
                Some(&path),
                "Replace the link with a project-owned regular file or directory.",
            ));
        }
        if metadata.is_dir() {
            collect_asset_paths(project_root, &path, paths)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(project_root).map_err(|_| {
                ProjectAssetImportError::new(
                    "project_asset_import.asset_path_escaped",
                    "Asset scan escaped the project root.",
                    Some(&path),
                    "Resolve the project containment violation.",
                )
            })?;
            let canonical = relative.to_string_lossy().replace('\\', "/");
            let folded = canonical.to_ascii_lowercase();
            if let Some(previous) = paths.insert(folded, canonical.clone()) {
                if previous != canonical {
                    return Err(ProjectAssetImportError::new(
                        "project_asset_import.existing_case_fold_collision",
                        format!("Existing project paths collide: {previous} and {canonical}."),
                        Some(&path),
                        "Resolve the project path collision before importing.",
                    ));
                }
            }
        }
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

fn asset_guid_for(project_id: &str, import_id: &str, asset_id: &str) -> String {
    let payload = format!("project-asset-guid.v1\0{project_id}\0{import_id}\0{asset_id}");
    format!(
        "asset-{}",
        sha256_prefixed(payload.as_bytes()).trim_start_matches("sha256:")
    )
}

fn validate_png(bytes: &[u8]) -> Result<(u32, u32), ProjectAssetImportError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.png_header_invalid",
            format!("PNG header cannot be decoded: {error}"),
            None,
            "Use a valid non-corrupt PNG image.",
        )
    })?;
    let width = reader.info().width;
    let height = reader.info().height;
    if width == 0
        || height == 0
        || width > MAX_TEXTURE_DIMENSION
        || height > MAX_TEXTURE_DIMENSION
        || reader.output_buffer_size() > MAX_DECODED_BYTES
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.png_dimensions_invalid",
            format!("PNG dimensions or decoded size exceed importer limits: {width}x{height}."),
            None,
            "Resize the image to a supported texture size.",
        ));
    }
    let mut decoded = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut decoded).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.png_decode_failed",
            format!("PNG payload cannot be decoded: {error}"),
            None,
            "Use a valid non-corrupt PNG image.",
        )
    })?;
    Ok((width, height))
}

fn validate_candidate_descriptor_and_meta(
    candidate: &ProjectAssetImportCandidate,
) -> Result<(), ProjectAssetImportError> {
    let root = Path::new(&candidate.revision.candidate_root);
    let descriptor_path = root.join(&candidate.record.descriptor_path);
    let descriptor: ImportedTextureDescriptor = read_json_file(&descriptor_path)?;
    if descriptor.asset_id != candidate.record.asset_id
        || descriptor.asset_guid != candidate.record.asset_guid
        || descriptor.source_image != candidate.record.source_path
        || digest_serializable(&descriptor.importer, "candidate texture settings")?
            != candidate.record.settings_hash
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.descriptor_binding_invalid",
            "Candidate texture descriptor does not match the AssetDB record.",
            Some(&descriptor_path),
            "Discard the invalid candidate and prepare it again.",
        ));
    }
    let meta_path = root.join(&candidate.record.meta_path);
    let meta: ProjectAssetMeta = read_json_file(&meta_path)?;
    if meta != candidate.meta
        || meta.asset_guid != candidate.record.asset_guid
        || meta.asset_id != candidate.record.asset_id
        || meta.source_hash != candidate.record.source_hash
        || meta.descriptor_path != candidate.record.descriptor_path
        || meta.source_path != candidate.record.source_path
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.meta_binding_invalid",
            "Candidate sidecar meta does not match the AssetDB record.",
            Some(&meta_path),
            "Discard the invalid candidate and prepare it again.",
        ));
    }
    Ok(())
}

fn read_json_file<T>(path: &Path) -> Result<T, ProjectAssetImportError>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.candidate_file_read_failed",
            format!("Candidate file cannot be read: {error}"),
            Some(path),
            "Discard the damaged candidate and prepare it again.",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.candidate_file_invalid",
            format!("Candidate JSON cannot be parsed: {error}"),
            Some(path),
            "Discard the invalid candidate and prepare it again.",
        )
    })
}

fn validate_candidate_record(
    candidate: &ProjectAssetImportCandidate,
) -> Result<(), ProjectAssetImportError> {
    if candidate.schema_version != PROJECT_ASSET_IMPORT_CANDIDATE_SCHEMA_VERSION
        || candidate.import_id.is_empty()
        || candidate.candidate_digest.is_empty()
        || candidate.candidate_digest != candidate_digest(candidate)?
        || candidate.record.asset_guid != candidate.meta.asset_guid
        || candidate.record.asset_id != candidate.meta.asset_id
        || candidate.record.source_hash != candidate.source_hash
        || candidate.record.source_byte_length != candidate.source_byte_length
        || candidate.database.asset_by_id(&candidate.record.asset_id) != Some(&candidate.record)
    {
        return Err(ProjectAssetImportError::new(
            "project_asset_import.candidate_record_invalid",
            "Asset import candidate record or digest binding is invalid.",
            Some(Path::new(&candidate.revision.candidate_root)),
            "Discard the candidate and prepare it again.",
        ));
    }
    Ok(())
}

fn candidate_digest(
    candidate: &ProjectAssetImportCandidate,
) -> Result<String, ProjectAssetImportError> {
    let mut normalized = candidate.clone();
    normalized.candidate_digest.clear();
    digest_serializable(&normalized, "asset import candidate")
}

fn validation_report_digest(
    report: &ProjectAssetImportValidationReport,
) -> Result<String, ProjectAssetImportError> {
    let mut normalized = report.clone();
    normalized.validation_digest.clear();
    digest_serializable(&normalized, "asset import validation report")
}

fn derived_state_digest(scope: &ProjectWriteScope) -> Result<String, ProjectAssetImportError> {
    let mut payload = b"project-asset-derived-state.v1\0".to_vec();
    for path in [ASSET_DATABASE_PATH, ASSET_GRAPH_PATH, ASSET_REGISTRY_PATH] {
        payload.extend_from_slice(&(path.len() as u64).to_le_bytes());
        payload.extend_from_slice(path.as_bytes());
        if scope.try_exists(path).map_err(project_write_error)? {
            let bytes = scope.read(path).map_err(project_write_error)?;
            payload.push(1);
            payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            payload.extend_from_slice(&bytes);
        } else {
            payload.push(0);
        }
    }
    Ok(sha256_prefixed(&payload))
}

fn derived_documents_digest(
    database: &AssetDatabaseDocument,
    graph: &AssetGraphDocument,
    registry: &AssetRegistryDocument,
) -> Result<String, ProjectAssetImportError> {
    let scope = [
        (ASSET_DATABASE_PATH, json_bytes(database)?),
        (ASSET_GRAPH_PATH, json_bytes(graph)?),
        (ASSET_REGISTRY_PATH, json_bytes(registry)?),
    ];
    let mut payload = b"project-asset-derived-state.v1\0".to_vec();
    for (path, bytes) in scope {
        payload.extend_from_slice(&(path.len() as u64).to_le_bytes());
        payload.extend_from_slice(path.as_bytes());
        payload.push(1);
        payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        payload.extend_from_slice(&bytes);
    }
    Ok(sha256_prefixed(&payload))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ProjectAssetImportError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ProjectAssetImportError::new(
            "project_asset_import.serialize_failed",
            format!("Asset import document cannot be serialized: {error}"),
            None,
            "Inspect the asset import schema implementation.",
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn digest_serializable<T: Serialize>(
    value: &T,
    role: &str,
) -> Result<String, ProjectAssetImportError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| {
            ProjectAssetImportError::new(
                "project_asset_import.digest_failed",
                format!("{role} cannot be serialized for digest: {error}"),
                None,
                "Inspect the schema implementation.",
            )
        })
}

fn project_write_error(error: ProjectWriteError) -> ProjectAssetImportError {
    ProjectAssetImportError {
        code: error.code.to_string(),
        message: error.to_string(),
        path: error.relative_path,
        next_action: "Resolve the project containment or filesystem error and retry.".to_string(),
    }
}

fn candidate_error(error: CandidateProjectRevisionError) -> ProjectAssetImportError {
    ProjectAssetImportError {
        code: error.code,
        message: error.message,
        path: error.path,
        next_action: error.next_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectLauncherState;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn project_asset_import_policy_rejects_invalid_identity_metadata_and_target() {
        assert_eq!(
            validate_asset_id("Player Ship").unwrap_err().code,
            "project_asset_import.asset_id_invalid"
        );
        assert_eq!(
            validate_asset_id("con").unwrap_err().code,
            "project_asset_import.asset_id_reserved"
        );
        assert_eq!(
            validate_target_directory("../Assets").unwrap_err().code,
            "project_write.path_parent_component"
        );
        assert_eq!(
            validate_target_directory("Scenes").unwrap_err().code,
            "project_asset_import.target_outside_assets"
        );
        let unknown = AssetLicenseMetadata {
            kind: AssetLicenseKind::Unknown,
            identifier: None,
            license_uri: None,
            attribution: None,
            note: None,
        };
        assert_eq!(
            validate_license_metadata(&unknown).unwrap_err().code,
            "project_asset_import.unknown_license_note_required"
        );
        let source = AssetImportSourceMetadata {
            kind: AssetImportSourceKind::Downloaded,
            source_uri: Some("not a uri".to_string()),
            creator: None,
            note: None,
        };
        assert_eq!(
            validate_source_metadata(&source).unwrap_err().code,
            "project_asset_import.metadata_uri_invalid"
        );
    }

    #[test]
    fn project_asset_import_database_rebuilds_graph_and_registry_deterministically() {
        let record = sample_record();
        let database = AssetDatabaseDocument {
            schema_version: PROJECT_ASSET_DATABASE_SCHEMA_VERSION.to_string(),
            project_id: "project-a".to_string(),
            database_version: 7,
            assets: vec![record.clone()],
        };
        validate_database(&database, "project-a").unwrap();
        let graph = build_graph(&database);
        let registry = build_registry(&database);
        assert_eq!(graph.built_from_database_version, 7);
        assert_eq!(graph.nodes[0].asset_guid, record.asset_guid);
        assert_eq!(registry.registry_version, 7);
        assert_eq!(registry.entries[0].source_hash, record.source_hash);
        assert_eq!(build_graph(&database), graph);
        assert_eq!(build_registry(&database), registry);
    }

    #[test]
    fn project_asset_import_prepare_stages_source_descriptor_meta_and_derived_state() {
        let fixture = fixture("prepare");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();

        assert_eq!(candidate.revision.changed_paths.len(), 3);
        assert!(candidate
            .revision
            .changed_paths
            .contains(&"Assets/Imported/test-texture.asset".to_string()));
        assert!(candidate.record.asset_guid.starts_with("asset-"));
        assert_eq!(candidate.database.database_version, 1);
        assert_eq!(candidate.graph.nodes.len(), 1);
        assert_eq!(candidate.registry.entries.len(), 1);
        assert!(!fixture.project.join(ASSET_DATABASE_PATH).exists());
        assert!(Path::new(&candidate.revision.candidate_root)
            .join(&candidate.record.source_path)
            .is_file());
    }

    #[test]
    fn project_asset_import_prepare_rejects_existing_target_and_case_collision() {
        let fixture = fixture("conflict");
        fs::create_dir_all(fixture.project.join("Assets/Imported")).unwrap();
        fs::write(
            fixture.project.join("Assets/Imported/Test-Texture.png"),
            b"collision",
        )
        .unwrap();

        let error = ProjectAssetImport::prepare(fixture.request()).unwrap_err();
        assert_eq!(error.code, "project_asset_import.case_fold_collision");
    }

    #[test]
    fn project_asset_import_validate_decodes_png_and_rejects_source_drift() {
        let fixture = fixture("validate");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        assert_eq!(report.status, ProjectAssetImportValidationStatus::Passed);
        assert_eq!((report.texture_width, report.texture_height), (1, 1));
        assert!(report.validation_digest.starts_with("sha256:"));

        fs::write(&fixture.source, png_bytes([0, 255, 0, 255])).unwrap();
        let error = ProjectAssetImport::validate(&candidate).unwrap_err();
        assert_eq!(error.code, "project_asset_import.source_drifted");
    }

    #[test]
    fn project_asset_import_validate_rejects_candidate_tamper() {
        let fixture = fixture("candidate-tamper");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        fs::write(
            Path::new(&candidate.revision.candidate_root).join(&candidate.record.source_path),
            b"not-png",
        )
        .unwrap();

        let error = ProjectAssetImport::validate(&candidate).unwrap_err();
        assert_eq!(error.code, "candidate_revision.candidate_digest_mismatch");
    }

    #[test]
    fn project_asset_import_apply_commits_source_and_derived_identity() {
        let fixture = fixture("apply");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let receipt = ProjectAssetImport::apply(apply_request(candidate.clone(), report)).unwrap();

        assert_eq!(receipt.changed_paths.len(), 6);
        assert_eq!(
            fs::read(fixture.project.join(&candidate.record.source_path)).unwrap(),
            fs::read(&fixture.source).unwrap()
        );
        assert_eq!(
            ProjectAssetImport::load_database(&fixture.project)
                .unwrap()
                .unwrap(),
            candidate.database
        );
        assert!(fixture
            .project
            .join(&receipt.rollback_record_path)
            .is_file());
        assert!(!fixture.project.join(ASSET_IMPORT_LOCK_PATH).exists());
    }

    #[test]
    fn project_asset_import_apply_failure_restores_all_before_state() {
        let fixture = fixture("apply-restore");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let record_path = rollback_record_path(&candidate);

        let error =
            ProjectAssetImport::apply_internal(apply_request(candidate.clone(), report), Some(1))
                .unwrap_err();

        assert_eq!(error.code, "project_asset_import.apply_failed_restored");
        for path in [
            candidate.record.source_path,
            candidate.record.descriptor_path,
            candidate.record.meta_path,
            ASSET_DATABASE_PATH.to_string(),
            ASSET_GRAPH_PATH.to_string(),
            ASSET_REGISTRY_PATH.to_string(),
        ] {
            assert!(!fixture.project.join(path).exists());
        }
        assert!(!fixture.project.join(record_path).exists());
        assert!(!fixture.project.join(ASSET_IMPORT_LOCK_PATH).exists());
    }

    #[test]
    fn project_asset_import_rollback_restores_before_state_and_removes_artifacts() {
        let fixture = fixture("rollback");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let receipt = ProjectAssetImport::apply(apply_request(candidate.clone(), report)).unwrap();

        let rollback = ProjectAssetImport::rollback(&receipt, &fixture.project).unwrap();

        assert!(rollback.rollback_record_removed);
        assert!(rollback.snapshot_files_removed);
        assert!(!fixture.project.join(&receipt.rollback_record_path).exists());
        assert!(!fixture.project.join(&candidate.record.source_path).exists());
        assert!(ProjectAssetImport::load_database(&fixture.project)
            .unwrap()
            .is_none());
        assert!(!fixture.project.join(ASSET_IMPORT_LOCK_PATH).exists());
    }

    #[test]
    fn project_asset_import_lock_rejects_concurrent_apply() {
        let fixture = fixture("lock");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let scope = ProjectWriteScope::open(&fixture.project).unwrap();
        let lock = scope
            .acquire_lock(ASSET_IMPORT_LOCK_PATH, b"other-import")
            .unwrap();

        let error = ProjectAssetImport::apply(apply_request(candidate, report)).unwrap_err();

        assert_eq!(error.code, "project_write.lock_held");
        lock.release().unwrap();
    }

    #[test]
    fn project_asset_import_rollback_rejects_record_tamper_and_applied_drift() {
        let record_fixture = fixture("rollback-record-tamper");
        let candidate = ProjectAssetImport::prepare(record_fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let receipt = ProjectAssetImport::apply(apply_request(candidate, report)).unwrap();
        fs::write(
            record_fixture.project.join(&receipt.rollback_record_path),
            b"not-json",
        )
        .unwrap();
        let error = ProjectAssetImport::rollback(&receipt, &record_fixture.project).unwrap_err();
        assert_eq!(error.code, "project_asset_import.rollback_record_invalid");

        let drift_fixture = fixture("rollback-applied-drift");
        let candidate = ProjectAssetImport::prepare(drift_fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let receipt = ProjectAssetImport::apply(apply_request(candidate.clone(), report)).unwrap();
        fs::write(
            drift_fixture.project.join(&candidate.record.source_path),
            png_bytes([0, 0, 255, 255]),
        )
        .unwrap();
        let error = ProjectAssetImport::rollback(&receipt, &drift_fixture.project).unwrap_err();
        assert_eq!(error.code, "project_asset_import.rollback_project_drifted");
    }

    #[test]
    fn project_asset_import_rollback_rejects_binary_snapshot_tamper_before_writing() {
        let fixture = fixture("rollback-snapshot-tamper");
        let initial = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let initial_report = ProjectAssetImport::validate(&initial).unwrap();
        ProjectAssetImport::apply(apply_request(initial.clone(), initial_report)).unwrap();

        fs::write(&fixture.source, png_bytes([0, 255, 0, 255])).unwrap();
        let mut replacement_request = fixture.request();
        replacement_request.import_id = "import-test-texture-replace".to_string();
        replacement_request.revision_id = "revision-test-texture-replace".to_string();
        replacement_request.conflict_policy = AssetImportConflictPolicy::ReplaceMatching {
            expected_asset_guid: initial.record.asset_guid,
            expected_source_hash: initial.record.source_hash,
        };
        let replacement = ProjectAssetImport::prepare(replacement_request).unwrap();
        let replacement_report = ProjectAssetImport::validate(&replacement).unwrap();
        let receipt =
            ProjectAssetImport::apply(apply_request(replacement.clone(), replacement_report))
                .unwrap();
        let record: AssetImportRollbackRecord = serde_json::from_slice(
            &fs::read(fixture.project.join(&receipt.rollback_record_path)).unwrap(),
        )
        .unwrap();
        let snapshot_path = record
            .snapshots
            .iter()
            .find_map(|snapshot| snapshot.snapshot_path.as_ref())
            .unwrap();
        let applied_source =
            fs::read(fixture.project.join(&replacement.record.source_path)).unwrap();
        fs::write(fixture.project.join(snapshot_path), b"tampered").unwrap();

        let error = ProjectAssetImport::rollback(&receipt, &fixture.project).unwrap_err();

        assert_eq!(
            error.code,
            "project_asset_import.rollback_snapshot_tampered"
        );
        assert_eq!(
            fs::read(fixture.project.join(&replacement.record.source_path)).unwrap(),
            applied_source
        );
    }

    #[test]
    fn project_asset_import_end_to_end_drives_browser_runtime_package_and_rollback() {
        let fixture = fixture("end-to-end");
        let candidate = ProjectAssetImport::prepare(fixture.request()).unwrap();
        let report = ProjectAssetImport::validate(&candidate).unwrap();
        let receipt = ProjectAssetImport::apply(apply_request(candidate.clone(), report)).unwrap();

        let browser = crate::AssetBrowserIndex::build(crate::AssetBrowserBuildRequest {
            project_root: fixture.project.clone(),
            query: editor_ui_model::AssetQuery::default(),
            selection: editor_ui_model::AssetSelection::default(),
        });
        let descriptor = browser
            .entries
            .iter()
            .find(|entry| entry.canonical_path == candidate.record.descriptor_path)
            .expect("imported descriptor should be visible");
        assert_eq!(
            descriptor.guid.as_deref(),
            Some(candidate.record.asset_guid.as_str())
        );
        assert!(!browser
            .entries
            .iter()
            .any(|entry| entry.canonical_path.ends_with(".meta.json")));

        let assembled = crate::ProjectRuntimePackageAssembler::assemble(
            crate::ProjectRuntimePackageAssemblyRequest::new(&fixture.project),
        );
        assert_eq!(
            assembled.status,
            crate::ProjectRuntimePackageAssemblyStatus::Success
        );
        let input = assembled.build_input.expect("runtime package build input");
        let runtime_asset = input
            .assets
            .iter()
            .find(|asset| asset.asset_id == candidate.record.asset_id)
            .expect("registered runtime asset");
        assert_eq!(
            runtime_asset.asset_guid.as_deref(),
            Some(candidate.record.asset_guid.as_str())
        );
        assert_eq!(runtime_asset.source, candidate.record.descriptor_path);
        let texture = input
            .texture_payloads
            .iter()
            .find(|texture| texture.metadata.asset_id == candidate.record.asset_id)
            .expect("cooked imported texture");
        assert_eq!((texture.metadata.width, texture.metadata.height), (1, 1));
        assert_eq!(texture.rgba8, vec![255, 0, 0, 255]);

        ProjectAssetImport::rollback(&receipt, &fixture.project).unwrap();
        assert!(ProjectAssetImport::load_database(&fixture.project)
            .unwrap()
            .is_none());
        assert!(!fixture
            .project
            .join(&candidate.record.descriptor_path)
            .exists());
    }

    fn apply_request(
        candidate: ProjectAssetImportCandidate,
        report: ProjectAssetImportValidationReport,
    ) -> ProjectAssetImportApplyRequest {
        ProjectAssetImportApplyRequest {
            approval: ProjectAssetImportApproval {
                schema_version: PROJECT_ASSET_IMPORT_APPROVAL_SCHEMA_VERSION.to_string(),
                approved_by: "test-maintainer".to_string(),
                candidate_digest: candidate.candidate_digest.clone(),
                validation_digest: report.validation_digest.clone(),
                allow_replace: matches!(
                    candidate.conflict_policy,
                    AssetImportConflictPolicy::ReplaceMatching { .. }
                ),
            },
            candidate,
            validation_report: report,
        }
    }

    fn sample_record() -> AssetDatabaseRecord {
        AssetDatabaseRecord {
            asset_guid: format!("asset-{}", "c".repeat(64)),
            asset_id: "texture-a".to_string(),
            display_name: "Texture A".to_string(),
            asset_type: "texture".to_string(),
            descriptor_path: "Assets/Imported/texture-a.asset".to_string(),
            source_path: "Assets/Imported/texture-a.png".to_string(),
            meta_path: "Assets/Imported/texture-a.asset.meta.json".to_string(),
            source_hash: format!("sha256:{}", "a".repeat(64)),
            source_byte_length: 4,
            importer_id: TEXTURE_IMPORTER_ID.to_string(),
            importer_version: TEXTURE_IMPORTER_VERSION,
            settings_hash: format!("sha256:{}", "b".repeat(64)),
            state: AssetDatabaseRecordState::Current,
            source_metadata: AssetImportSourceMetadata::local_file(),
            license: AssetLicenseMetadata::project_owned(),
            direct_dependencies: Vec::new(),
        }
    }

    struct ImportFixture {
        root: PathBuf,
        project: PathBuf,
        source: PathBuf,
        candidates: PathBuf,
    }

    impl ImportFixture {
        fn request(&self) -> ProjectAssetImportPrepareRequest {
            ProjectAssetImportPrepareRequest {
                import_id: "import-test-texture".to_string(),
                revision_id: "revision-test-texture".to_string(),
                project_root: self.project.clone(),
                candidate_store_root: self.candidates.clone(),
                source_path: self.source.clone(),
                target_directory: "Assets/Imported".to_string(),
                asset_id: "test-texture".to_string(),
                display_name: "Test Texture".to_string(),
                conflict_policy: AssetImportConflictPolicy::RejectExisting,
                source_metadata: AssetImportSourceMetadata::local_file(),
                license: AssetLicenseMetadata::project_owned(),
                texture_settings: TextureImportSettings::default(),
            }
        }
    }

    impl Drop for ImportFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(label: &str) -> ImportFixture {
        let root = test_root(label);
        let project = root.join("project");
        let source_dir = root.join("external");
        let candidates = root.join("candidates");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&candidates).unwrap();
        ProjectLauncherState::new("0.0.1")
            .create_project(&project, "Asset Import Test")
            .unwrap();
        let source = source_dir.join("test-texture.png");
        fs::write(&source, png_bytes([255, 0, 0, 255])).unwrap();
        ImportFixture {
            root,
            project,
            source,
            candidates,
        }
    }

    fn png_bytes(pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&pixel).unwrap();
        }
        bytes
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aife-project-asset-import-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
