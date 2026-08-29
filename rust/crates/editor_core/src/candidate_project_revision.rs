use crate::ProjectRelativePath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

pub const CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION: &str = "candidate-project-revision.v1";
const PROJECT_TREE_DIGEST_SCHEMA_VERSION: &str = "project-tree-digest.v1";
const PROJECT_DIGEST_IO_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug)]
struct ProjectSourceFile {
    relative: String,
    path: PathBuf,
    length: u64,
}

struct ProjectDigestSourcePolicy;

impl ProjectDigestSourcePolicy {
    fn excludes_root_path(relative: &Path) -> bool {
        let mut components = relative.components();
        let Some(Component::Normal(first)) = components.next() else {
            return false;
        };
        if first
            .to_string_lossy()
            .eq_ignore_ascii_case("RuntimeModule")
            && components.next().is_some_and(|component| {
                matches!(component, Component::Normal(value) if value.to_string_lossy().eq_ignore_ascii_case("target"))
            })
        {
            return true;
        }
        matches!(
            first.to_string_lossy().to_ascii_lowercase().as_str(),
            "library" | "build" | "target" | ".git" | ".aife" | ".aife-candidates"
        )
    }

    fn directory_is_cargo_root(directory: &Path) -> bool {
        regular_file_without_reparse(&directory.join("Cargo.toml"))
    }

    fn excludes_directory(relative: &Path, path: &Path, inside_cargo_tree: bool) -> bool {
        Self::excludes_root_path(relative)
            || (inside_cargo_tree
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("target")))
    }

    fn excludes_change_path(project_root: &Path, relative: &Path) -> bool {
        if Self::excludes_root_path(relative) {
            return true;
        }
        let mut ancestor = project_root.to_path_buf();
        let mut inside_cargo_tree = false;
        for component in relative.components() {
            let Component::Normal(value) = component else {
                return false;
            };
            inside_cargo_tree = inside_cargo_tree || Self::directory_is_cargo_root(&ancestor);
            if value.to_string_lossy().eq_ignore_ascii_case("target") && inside_cargo_tree {
                return true;
            }
            ancestor.push(value);
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateFileChange {
    CreateOrReplace { path: String, bytes: Vec<u8> },
    Delete { path: String },
}

impl CandidateFileChange {
    fn path(&self) -> &str {
        match self {
            Self::CreateOrReplace { path, .. } | Self::Delete { path } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateProjectRevisionRequest {
    pub revision_id: String,
    pub project_root: PathBuf,
    pub candidate_store_root: PathBuf,
    pub changes: Vec<CandidateFileChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateProjectRevisionStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateProjectRevision {
    pub schema_version: String,
    pub revision_id: String,
    pub status: CandidateProjectRevisionStatus,
    pub project_root: String,
    pub candidate_root: String,
    pub base_project_digest: String,
    pub candidate_project_digest: String,
    pub changed_paths: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateBaseVerificationStatus {
    Matched,
    Drifted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateBaseVerification {
    pub revision_id: String,
    pub status: CandidateBaseVerificationStatus,
    pub expected_digest: String,
    pub actual_digest: String,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDiscardOutcome {
    Removed,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateDiscardReceipt {
    pub revision_id: String,
    pub outcome: CandidateDiscardOutcome,
    pub candidate_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateProjectRevisionError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

impl CandidateProjectRevisionError {
    fn new(
        code: &'static str,
        message: impl Into<String>,
        path: Option<&Path>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            path: path.map(|value| value.display().to_string()),
            next_action: next_action.into(),
        }
    }
}

impl std::fmt::Display for CandidateProjectRevisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CandidateProjectRevisionError {}

pub struct CandidateProjectRevisionStore;

impl CandidateProjectRevisionStore {
    pub fn project_digest(
        project_root: impl AsRef<Path>,
    ) -> Result<String, CandidateProjectRevisionError> {
        Self::project_digest_cancellable(project_root, || false)
    }

    pub fn project_digest_cancellable(
        project_root: impl AsRef<Path>,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<String, CandidateProjectRevisionError> {
        let project_root = canonical_directory(project_root.as_ref(), "project_root")?;
        project_tree_digest_cancellable(&project_root, &is_cancelled)
    }

    pub fn stage(
        request: CandidateProjectRevisionRequest,
    ) -> Result<CandidateProjectRevision, CandidateProjectRevisionError> {
        validate_revision_id(&request.revision_id)?;
        let project_root = canonical_directory(&request.project_root, "project_root")?;
        let store_root = prepare_candidate_store(&project_root, &request.candidate_store_root)?;

        let candidate_root = store_root.join(&request.revision_id);
        if candidate_root.exists() {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.already_exists",
                "Candidate revision root already exists.",
                Some(&candidate_root),
                "Use a new revision id or discard the existing candidate explicitly.",
            ));
        }
        let staging_root = store_root.join(format!(".{}.staging", request.revision_id));
        if staging_root.exists() {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.staging_exists",
                "Candidate staging root already exists.",
                Some(&staging_root),
                "Inspect and remove the stale staging root before retrying.",
            ));
        }

        let result = (|| {
            let base_project_digest = project_tree_digest(&project_root)?;
            copy_project_tree(&project_root, &staging_root, &project_root)?;
            apply_changes(&staging_root, request.changes)?;
            let stable_base_digest = project_tree_digest(&project_root)?;
            if stable_base_digest != base_project_digest {
                return Err(CandidateProjectRevisionError::new(
                    "candidate_revision.base_changed_during_stage",
                    "Project content changed while the candidate was being staged.",
                    Some(&project_root),
                    "Rescan the project and create a new candidate from the latest base.",
                ));
            }
            let changed_paths = changed_paths_between(&project_root, &staging_root)?;
            let candidate_project_digest = project_tree_digest(&staging_root)?;
            let final_base_digest = project_tree_digest(&project_root)?;
            if final_base_digest != base_project_digest {
                return Err(CandidateProjectRevisionError::new(
                    "candidate_revision.base_changed_during_stage",
                    "Project content changed while the candidate was being finalized.",
                    Some(&project_root),
                    "Rescan the project and create a new candidate from the latest base.",
                ));
            }
            fs::rename(&staging_root, &candidate_root).map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.publish_failed",
                    format!("Candidate staging publish failed: {error}"),
                    Some(&candidate_root),
                    "Check candidate store permissions and retry with a new revision id.",
                )
            })?;
            Ok(CandidateProjectRevision {
                schema_version: CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION.to_string(),
                revision_id: request.revision_id,
                status: CandidateProjectRevisionStatus::Ready,
                project_root: project_root.display().to_string(),
                candidate_root: candidate_root.display().to_string(),
                base_project_digest,
                candidate_project_digest,
                changed_paths,
                diagnostics: Vec::new(),
                next_actions: vec![
                    "Validate the candidate revision before requesting approval.".to_string(),
                ],
            })
        })();

        if result.is_err() && staging_root.exists() {
            let _ = fs::remove_dir_all(&staging_root);
        }
        result
    }

    pub fn verify_base(
        revision: &CandidateProjectRevision,
        project_root: &Path,
    ) -> Result<CandidateBaseVerification, CandidateProjectRevisionError> {
        validate_revision(revision)?;
        let project_root = canonical_directory(project_root, "project_root")?;
        let expected_root =
            canonical_directory(Path::new(&revision.project_root), "revision_root")?;
        if !paths_equal(&project_root, &expected_root) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.project_root_mismatch",
                "Verification project root does not match the revision base root.",
                Some(&project_root),
                "Verify the candidate against its original project root.",
            ));
        }
        let candidate_root =
            canonical_directory(Path::new(&revision.candidate_root), "candidate_root")?;
        if candidate_root.file_name().and_then(|value| value.to_str())
            != Some(revision.revision_id.as_str())
        {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.candidate_root_mismatch",
                "Recorded candidate root does not match the revision id.",
                Some(&candidate_root),
                "Discard the invalid record and recreate the candidate revision.",
            ));
        }
        let actual_candidate_digest = project_tree_digest(&candidate_root)?;
        if actual_candidate_digest != revision.candidate_project_digest {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.candidate_digest_mismatch",
                "Candidate content no longer matches the recorded candidate digest.",
                Some(&candidate_root),
                "Discard the modified candidate and create a new revision.",
            ));
        }
        let actual_digest = project_tree_digest(&project_root)?;
        let status = if actual_digest == revision.base_project_digest {
            let actual_changed_paths = changed_paths_between(&project_root, &candidate_root)?;
            if actual_changed_paths != revision.changed_paths {
                return Err(CandidateProjectRevisionError::new(
                    "candidate_revision.changed_paths_mismatch",
                    "Recorded changed paths do not match the staged candidate diff.",
                    Some(&candidate_root),
                    "Discard the invalid record and recreate the candidate revision.",
                ));
            }
            CandidateBaseVerificationStatus::Matched
        } else {
            CandidateBaseVerificationStatus::Drifted
        };
        let drifted = status == CandidateBaseVerificationStatus::Drifted;
        Ok(CandidateBaseVerification {
            revision_id: revision.revision_id.clone(),
            status,
            expected_digest: revision.base_project_digest.clone(),
            actual_digest,
            diagnostics: if drifted {
                vec!["candidate_revision.base_drifted".to_string()]
            } else {
                Vec::new()
            },
            next_actions: if drifted {
                vec!["Discard or rebase the candidate before approval or apply.".to_string()]
            } else {
                vec!["The candidate may proceed to validation.".to_string()]
            },
        })
    }

    pub fn discard(
        revision: &CandidateProjectRevision,
        candidate_store_root: &Path,
    ) -> Result<CandidateDiscardReceipt, CandidateProjectRevisionError> {
        validate_revision(revision)?;
        let store_root = canonical_directory(candidate_store_root, "store_root")?;
        let expected_root = store_root.join(&revision.revision_id);
        let recorded_root = PathBuf::from(&revision.candidate_root);
        if !paths_equal_lexical(&expected_root, &recorded_root) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.discard_root_mismatch",
                "Recorded candidate root is not the exact revision root under this store.",
                Some(&recorded_root),
                "Use the candidate store that owns this revision.",
            ));
        }
        let outcome = if expected_root.exists() {
            let canonical_candidate = expected_root.canonicalize().map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.discard_inspection_failed",
                    format!("Candidate root cannot be inspected: {error}"),
                    Some(&expected_root),
                    "Inspect the candidate root before retrying discard.",
                )
            })?;
            if canonical_candidate.parent() != Some(store_root.as_path()) {
                return Err(CandidateProjectRevisionError::new(
                    "candidate_revision.discard_escaped_store",
                    "Candidate root escaped its owning store.",
                    Some(&canonical_candidate),
                    "Resolve the candidate store containment violation manually.",
                ));
            }
            fs::remove_dir_all(&canonical_candidate).map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.discard_failed",
                    format!("Candidate revision cannot be removed: {error}"),
                    Some(&canonical_candidate),
                    "Close processes using the candidate and retry.",
                )
            })?;
            CandidateDiscardOutcome::Removed
        } else {
            CandidateDiscardOutcome::Missing
        };
        Ok(CandidateDiscardReceipt {
            revision_id: revision.revision_id.clone(),
            outcome,
            candidate_root: expected_root.display().to_string(),
        })
    }
}

fn validate_revision(
    revision: &CandidateProjectRevision,
) -> Result<(), CandidateProjectRevisionError> {
    if revision.schema_version != CANDIDATE_PROJECT_REVISION_SCHEMA_VERSION {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.schema_unsupported",
            format!(
                "Unsupported candidate revision schema: {}",
                revision.schema_version
            ),
            None,
            "Migrate or recreate the candidate revision.",
        ));
    }
    validate_revision_id(&revision.revision_id)
        .and_then(|_| validate_digest(&revision.base_project_digest, "base_project_digest"))
        .and_then(|_| {
            validate_digest(
                &revision.candidate_project_digest,
                "candidate_project_digest",
            )
        })?;
    let project_root = Path::new(&revision.project_root);
    let candidate_root = Path::new(&revision.candidate_root);
    if !project_root.is_absolute() || !candidate_root.is_absolute() {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.recorded_root_invalid",
            "Candidate revision roots must be absolute paths.",
            None,
            "Discard the invalid record and recreate the candidate revision.",
        ));
    }
    let mut previous: Option<&str> = None;
    for path in &revision.changed_paths {
        let parsed = ProjectRelativePath::parse(path).map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.changed_path_invalid",
                format!("Recorded changed path is invalid: {error}"),
                None,
                "Discard the invalid record and recreate the candidate revision.",
            )
        })?;
        if parsed.to_string() != *path || previous.is_some_and(|value| value >= path.as_str()) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.changed_paths_noncanonical",
                "Recorded changed paths must be canonical, unique, and sorted.",
                None,
                "Discard the invalid record and recreate the candidate revision.",
            ));
        }
        previous = Some(path.as_str());
    }
    Ok(())
}

fn validate_digest(digest: &str, role: &str) -> Result<(), CandidateProjectRevisionError> {
    let valid = digest.len() == 71
        && digest.starts_with("sha256:")
        && digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(CandidateProjectRevisionError::new(
            "candidate_revision.digest_invalid",
            format!("Recorded {role} is not a canonical SHA-256 digest."),
            None,
            "Discard the invalid record and recreate the candidate revision.",
        ))
    }
}

fn validate_revision_id(revision_id: &str) -> Result<(), CandidateProjectRevisionError> {
    if revision_id.is_empty()
        || revision_id.len() > 96
        || !revision_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.id_invalid",
            "Revision id must contain 1-96 ASCII letters, digits, '-' or '_'.",
            None,
            "Generate a canonical opaque revision id.",
        ));
    }
    Ok(())
}

fn canonical_directory(path: &Path, role: &str) -> Result<PathBuf, CandidateProjectRevisionError> {
    let canonical = path.canonicalize().map_err(|error| {
        CandidateProjectRevisionError::new(
            "candidate_revision.root_unavailable",
            format!("{role} is unavailable: {error}"),
            Some(path),
            "Provide an existing accessible directory.",
        )
    })?;
    if !canonical.is_dir() {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.root_not_directory",
            format!("{role} is not a directory."),
            Some(&canonical),
            "Provide an existing accessible directory.",
        ));
    }
    Ok(canonical)
}

fn reject_overlapping_roots(
    project_root: &Path,
    store_root: &Path,
) -> Result<(), CandidateProjectRevisionError> {
    if path_starts_with(project_root, store_root) || path_starts_with(store_root, project_root) {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.roots_overlap",
            "Project root and candidate store root must not contain one another.",
            Some(store_root),
            "Choose a dedicated candidate store outside the project tree.",
        ));
    }
    Ok(())
}

fn prepare_candidate_store(
    project_root: &Path,
    requested_store_root: &Path,
) -> Result<PathBuf, CandidateProjectRevisionError> {
    if requested_store_root.exists() {
        let store_root = canonical_directory(requested_store_root, "store_root")?;
        reject_overlapping_roots(project_root, &store_root)?;
        return Ok(store_root);
    }

    let absolute_request = if requested_store_root.is_absolute() {
        requested_store_root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.current_directory_unavailable",
                    format!("Current directory cannot be resolved: {error}"),
                    None,
                    "Use an absolute candidate store path.",
                )
            })?
            .join(requested_store_root)
    };
    let proposed_store_root = canonicalize_with_missing_suffix(&absolute_request)?;
    reject_overlapping_roots(project_root, &proposed_store_root)?;
    fs::create_dir_all(&proposed_store_root).map_err(|error| {
        CandidateProjectRevisionError::new(
            "candidate_revision.store_unavailable",
            format!("Candidate store cannot be created: {error}"),
            Some(&proposed_store_root),
            "Choose a writable candidate store outside the project root.",
        )
    })?;
    let store_root = canonical_directory(&proposed_store_root, "store_root")?;
    reject_overlapping_roots(project_root, &store_root)?;
    Ok(store_root)
}

fn canonicalize_with_missing_suffix(path: &Path) -> Result<PathBuf, CandidateProjectRevisionError> {
    let mut ancestor = path;
    let mut missing = Vec::new();
    while !ancestor.exists() {
        let leaf = ancestor.file_name().ok_or_else(|| {
            CandidateProjectRevisionError::new(
                "candidate_revision.store_path_invalid",
                "Candidate store path has no resolvable existing ancestor.",
                Some(path),
                "Provide a candidate store below an accessible filesystem root.",
            )
        })?;
        missing.push(leaf.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| {
            CandidateProjectRevisionError::new(
                "candidate_revision.store_path_invalid",
                "Candidate store path has no resolvable parent.",
                Some(path),
                "Provide a candidate store below an accessible filesystem root.",
            )
        })?;
    }
    let mut resolved = canonical_directory(ancestor, "store_ancestor")?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn project_tree_digest(root: &Path) -> Result<String, CandidateProjectRevisionError> {
    project_tree_digest_cancellable(root, &|| false)
}

fn project_tree_digest_cancellable(
    root: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<String, CandidateProjectRevisionError> {
    reject_cancelled(root, is_cancelled)?;
    let files = project_source_files_cancellable(root, is_cancelled)?;
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_TREE_DIGEST_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    for file in files {
        reject_cancelled(&file.path, is_cancelled)?;
        hasher.update((file.relative.len() as u64).to_le_bytes());
        hasher.update(file.relative.as_bytes());
        hasher.update(file.length.to_le_bytes());
        stream_file_into_digest_cancellable(&file, &mut hasher, is_cancelled)?;
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn project_source_files(
    root: &Path,
) -> Result<Vec<ProjectSourceFile>, CandidateProjectRevisionError> {
    project_source_files_cancellable(root, &|| false)
}

fn project_source_files_cancellable(
    root: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<Vec<ProjectSourceFile>, CandidateProjectRevisionError> {
    let mut files = Vec::new();
    collect_project_files(root, root, false, &mut files, is_cancelled)?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn stream_file_into_digest(
    file: &ProjectSourceFile,
    hasher: &mut Sha256,
) -> Result<(), CandidateProjectRevisionError> {
    stream_file_into_digest_cancellable(file, hasher, &|| false)
}

fn stream_file_into_digest_cancellable(
    file: &ProjectSourceFile,
    hasher: &mut Sha256,
    is_cancelled: &impl Fn() -> bool,
) -> Result<(), CandidateProjectRevisionError> {
    let source = fs::File::open(&file.path).map_err(|error| {
        CandidateProjectRevisionError::new(
            "candidate_revision.file_read_failed",
            format!("Project file cannot be opened: {error}"),
            Some(&file.path),
            "Restore readable project source files before staging.",
        )
    })?;
    let mut reader = BufReader::with_capacity(PROJECT_DIGEST_IO_BUFFER_BYTES, source);
    let mut buffer = [0_u8; PROJECT_DIGEST_IO_BUFFER_BYTES];
    let mut consumed = 0_u64;
    loop {
        reject_cancelled(&file.path, is_cancelled)?;
        let read = reader.read(&mut buffer).map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.file_read_failed",
                format!("Project file cannot be read: {error}"),
                Some(&file.path),
                "Restore readable project source files before staging.",
            )
        })?;
        if read == 0 {
            break;
        }
        consumed = consumed.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    let final_length = fs::metadata(&file.path)
        .map(|metadata| metadata.len())
        .map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.file_metadata_failed",
                format!("Project file metadata cannot be re-read: {error}"),
                Some(&file.path),
                "Retry from a stable project source tree.",
            )
        })?;
    if consumed != file.length || final_length != file.length {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.file_changed_during_digest",
            "Project file length changed while its digest was being computed.",
            Some(&file.path),
            "Retry after project source writes have completed.",
        ));
    }
    Ok(())
}

fn project_file_hashes(
    root: &Path,
) -> Result<BTreeMap<String, String>, CandidateProjectRevisionError> {
    project_source_files(root)?
        .into_iter()
        .map(|file| {
            let relative = file.relative.clone();
            let mut hasher = Sha256::new();
            stream_file_into_digest(&file, &mut hasher)?;
            Ok((relative, format!("sha256:{:x}", hasher.finalize())))
        })
        .collect()
}

fn changed_paths_between(
    base_root: &Path,
    candidate_root: &Path,
) -> Result<Vec<String>, CandidateProjectRevisionError> {
    let base = project_file_hashes(base_root)?;
    let candidate = project_file_hashes(candidate_root)?;
    Ok(base
        .keys()
        .chain(candidate.keys())
        .filter(|path| base.get(*path) != candidate.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn collect_project_files(
    root: &Path,
    directory: &Path,
    inside_cargo_tree: bool,
    files: &mut Vec<ProjectSourceFile>,
    is_cancelled: &impl Fn() -> bool,
) -> Result<(), CandidateProjectRevisionError> {
    reject_cancelled(directory, is_cancelled)?;
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.directory_read_failed",
                format!("Project directory cannot be read: {error}"),
                Some(directory),
                "Restore readable project directories before staging.",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.directory_entry_failed",
                format!("Project directory entry cannot be read: {error}"),
                Some(directory),
                "Restore readable project directories before staging.",
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    let inside_cargo_tree =
        inside_cargo_tree || ProjectDigestSourcePolicy::directory_is_cargo_root(directory);
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| {
            CandidateProjectRevisionError::new(
                "candidate_revision.source_escaped_root",
                "Project source escaped the project root.",
                Some(&path),
                "Resolve the project containment violation.",
            )
        })?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.source_metadata_failed",
                format!("Project source metadata cannot be read: {error}"),
                Some(&path),
                "Restore a regular project source tree.",
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.source_link_rejected",
                "Project candidate staging does not follow symbolic links or junctions.",
                Some(&path),
                "Replace the link with a project-owned regular file or directory.",
            ));
        }
        if metadata.is_dir() {
            if ProjectDigestSourcePolicy::excludes_directory(relative, &path, inside_cargo_tree) {
                continue;
            }
            collect_project_files(root, &path, inside_cargo_tree, files, is_cancelled)?;
        } else if metadata.is_file() {
            if ProjectDigestSourcePolicy::excludes_root_path(relative) {
                continue;
            }
            files.push(ProjectSourceFile {
                relative: canonical_relative(relative)?,
                path,
                length: metadata.len(),
            });
        } else {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.source_type_rejected",
                "Project source contains an unsupported filesystem entry.",
                Some(&path),
                "Remove the unsupported filesystem entry.",
            ));
        }
    }
    Ok(())
}

fn reject_cancelled(
    path: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<(), CandidateProjectRevisionError> {
    if is_cancelled() {
        return Err(CandidateProjectRevisionError::new(
            "candidate_revision.digest_cancelled",
            "Project digest computation was cancelled.",
            Some(path),
            "Retry opening the project when ready.",
        ));
    }
    Ok(())
}

fn copy_project_tree(
    source_root: &Path,
    destination_root: &Path,
    source_directory: &Path,
) -> Result<(), CandidateProjectRevisionError> {
    copy_project_tree_inner(source_root, destination_root, source_directory, false)
}

fn copy_project_tree_inner(
    source_root: &Path,
    destination_root: &Path,
    source_directory: &Path,
    inside_cargo_tree: bool,
) -> Result<(), CandidateProjectRevisionError> {
    fs::create_dir_all(destination_root).map_err(|error| {
        CandidateProjectRevisionError::new(
            "candidate_revision.staging_create_failed",
            format!("Candidate staging directory cannot be created: {error}"),
            Some(destination_root),
            "Choose a writable candidate store.",
        )
    })?;
    let mut entries = fs::read_dir(source_directory)
        .map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.copy_read_failed",
                format!("Project directory cannot be copied: {error}"),
                Some(source_directory),
                "Restore readable project source before retrying.",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.copy_entry_failed",
                format!("Project directory entry cannot be copied: {error}"),
                Some(source_directory),
                "Restore readable project source before retrying.",
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    let inside_cargo_tree =
        inside_cargo_tree || ProjectDigestSourcePolicy::directory_is_cargo_root(source_directory);
    for entry in entries {
        let source = entry.path();
        let relative = source.strip_prefix(source_root).map_err(|_| {
            CandidateProjectRevisionError::new(
                "candidate_revision.copy_escaped_root",
                "Project copy source escaped its root.",
                Some(&source),
                "Resolve the project containment violation.",
            )
        })?;
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.copy_metadata_failed",
                format!("Project copy metadata cannot be read: {error}"),
                Some(&source),
                "Restore a regular project source tree.",
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.source_link_rejected",
                "Project candidate staging does not follow symbolic links or junctions.",
                Some(&source),
                "Replace the link with a project-owned regular file or directory.",
            ));
        }
        let destination = destination_root.join(relative);
        if metadata.is_dir() {
            if ProjectDigestSourcePolicy::excludes_directory(relative, &source, inside_cargo_tree) {
                continue;
            }
            fs::create_dir_all(&destination).map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.copy_directory_failed",
                    format!("Candidate directory cannot be created: {error}"),
                    Some(&destination),
                    "Check candidate store permissions.",
                )
            })?;
            copy_project_tree_inner(source_root, destination_root, &source, inside_cargo_tree)?;
        } else if metadata.is_file() {
            if ProjectDigestSourcePolicy::excludes_root_path(relative) {
                continue;
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    CandidateProjectRevisionError::new(
                        "candidate_revision.copy_directory_failed",
                        format!("Candidate directory cannot be created: {error}"),
                        Some(parent),
                        "Check candidate store permissions.",
                    )
                })?;
            }
            fs::copy(&source, &destination).map_err(|error| {
                CandidateProjectRevisionError::new(
                    "candidate_revision.copy_file_failed",
                    format!("Project file cannot be copied: {error}"),
                    Some(&source),
                    "Check project readability and candidate store capacity.",
                )
            })?;
        }
    }
    Ok(())
}

fn apply_changes(
    candidate_root: &Path,
    changes: Vec<CandidateFileChange>,
) -> Result<(), CandidateProjectRevisionError> {
    for change in changes {
        let relative = ProjectRelativePath::parse(change.path()).map_err(|error| {
            CandidateProjectRevisionError::new(
                "candidate_revision.change_path_invalid",
                format!("Candidate change path is invalid: {error}"),
                None,
                "Use a canonical project-relative change path.",
            )
        })?;
        if ProjectDigestSourcePolicy::excludes_change_path(candidate_root, relative.as_path()) {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.change_generated_path_rejected",
                "Candidate changes cannot target generated or revision metadata directories.",
                Some(relative.as_path()),
                "Target a project source path.",
            ));
        }
        let destination = candidate_root.join(relative.as_path());
        match change {
            CandidateFileChange::CreateOrReplace { bytes, .. } => {
                if destination.is_dir() {
                    return Err(CandidateProjectRevisionError::new(
                        "candidate_revision.change_target_is_directory",
                        "Candidate file change targets an existing directory.",
                        Some(&destination),
                        "Choose a file path for the candidate change.",
                    ));
                }
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        CandidateProjectRevisionError::new(
                            "candidate_revision.change_parent_failed",
                            format!("Candidate change parent cannot be created: {error}"),
                            Some(parent),
                            "Check candidate store permissions.",
                        )
                    })?;
                }
                fs::write(&destination, bytes).map_err(|error| {
                    CandidateProjectRevisionError::new(
                        "candidate_revision.change_write_failed",
                        format!("Candidate change cannot be written: {error}"),
                        Some(&destination),
                        "Check candidate store permissions and capacity.",
                    )
                })?;
            }
            CandidateFileChange::Delete { .. } => {
                if destination.is_dir() {
                    return Err(CandidateProjectRevisionError::new(
                        "candidate_revision.delete_directory_rejected",
                        "Candidate delete only supports files.",
                        Some(&destination),
                        "Delete files explicitly; directory cleanup is derived.",
                    ));
                }
                if destination.exists() {
                    fs::remove_file(&destination).map_err(|error| {
                        CandidateProjectRevisionError::new(
                            "candidate_revision.change_delete_failed",
                            format!("Candidate file cannot be deleted: {error}"),
                            Some(&destination),
                            "Check candidate store permissions.",
                        )
                    })?;
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

fn regular_file_without_reparse(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.is_file() && !is_link_or_reparse(&metadata))
}

fn canonical_relative(path: &Path) -> Result<String, CandidateProjectRevisionError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(CandidateProjectRevisionError::new(
                "candidate_revision.relative_path_invalid",
                "Project source path is not canonical relative syntax.",
                Some(path),
                "Normalize the project source tree.",
            ));
        };
        let value = value.to_str().ok_or_else(|| {
            CandidateProjectRevisionError::new(
                "candidate_revision.non_utf8_path_rejected",
                "Project source contains a non-UTF-8 path.",
                Some(path),
                "Rename the path using valid UTF-8 characters.",
            )
        })?;
        parts.push(value);
    }
    Ok(parts.join("/"))
}

#[cfg(windows)]
fn comparable_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(rest) = normalized.strip_prefix(r"\\?\unc\") {
        format!(r"\\{rest}")
    } else {
        normalized
            .strip_prefix(r"\\?\")
            .unwrap_or(&normalized)
            .to_string()
    }
}

#[cfg(not(windows))]
fn comparable_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn path_starts_with(path: &Path, base: &Path) -> bool {
    let path = comparable_path(path);
    let base = comparable_path(base);
    path == base
        || if base.ends_with(std::path::MAIN_SEPARATOR) {
            path.starts_with(&base)
        } else {
            path.starts_with(&(base + std::path::MAIN_SEPARATOR_STR))
        }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn paths_equal_lexical(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn digest_is_stable_and_excludes_generated_directories() {
        let root = fixture_project("digest");
        let first = project_tree_digest(&root).unwrap();
        fs::create_dir_all(root.join("Library/cache")).unwrap();
        fs::write(root.join("Library/cache/value.bin"), b"generated").unwrap();
        fs::create_dir_all(root.join("Build/Windows")).unwrap();
        fs::write(root.join("Build/Windows/Game.exe"), b"generated").unwrap();
        fs::create_dir_all(root.join(".aife/editor-preview/dev/main")).unwrap();
        fs::write(
            root.join(".aife/editor-preview/dev/main/cache-manifest.json"),
            b"generated",
        )
        .unwrap();
        fs::create_dir_all(root.join(".aife/reports/release-package")).unwrap();
        fs::write(
            root.join(".aife/reports/release-package/latest.json"),
            b"generated",
        )
        .unwrap();
        fs::create_dir_all(root.join("RuntimeModule/target/debug")).unwrap();
        fs::write(
            root.join("RuntimeModule/target/debug/project.dll"),
            b"generated",
        )
        .unwrap();
        assert_eq!(first, project_tree_digest(&root).unwrap());
        fs::write(root.join("Scenes/Main.scene.json"), b"changed").unwrap();
        assert_ne!(first, project_tree_digest(&root).unwrap());
    }

    #[test]
    fn digest_excludes_nested_cargo_targets_without_hiding_project_source() {
        let root = fixture_project("nested-cargo-target");
        let cargo_root = root.join("Tests/FeatureHarness");
        fs::create_dir_all(cargo_root.join("src")).unwrap();
        fs::write(
            cargo_root.join("Cargo.toml"),
            b"[package]\nname = \"feature_harness\"\nversion = \"0.0.3\"\n",
        )
        .unwrap();
        fs::write(cargo_root.join("src/lib.rs"), b"pub fn source() {}\n").unwrap();

        let before_target = project_tree_digest(&root).unwrap();
        fs::create_dir_all(cargo_root.join("target/debug/deps")).unwrap();
        fs::write(
            cargo_root.join("target/debug/deps/generated.rlib"),
            vec![0x5a; 1024 * 1024],
        )
        .unwrap();
        assert_eq!(before_target, project_tree_digest(&root).unwrap());

        fs::write(
            cargo_root.join("src/lib.rs"),
            b"pub fn changed_source() {}\n",
        )
        .unwrap();
        assert_ne!(before_target, project_tree_digest(&root).unwrap());

        let ordinary_target = root.join("Assets/target");
        fs::create_dir_all(&ordinary_target).unwrap();
        let before_project_source = project_tree_digest(&root).unwrap();
        fs::write(ordinary_target.join("authored.asset"), b"project-owned").unwrap();
        assert_ne!(before_project_source, project_tree_digest(&root).unwrap());
    }

    #[test]
    fn streaming_digest_preserves_the_canonical_project_tree_protocol() {
        let root = fixture_project("streaming-protocol");
        fs::create_dir_all(root.join("Assets/Large")).unwrap();
        fs::write(
            root.join("Assets/Large/payload.bin"),
            vec![0x3c; PROJECT_DIGEST_IO_BUFFER_BYTES * 3 + 17],
        )
        .unwrap();

        assert_eq!(
            project_tree_digest(&root).unwrap(),
            in_memory_project_tree_digest_for_test(&root)
        );
    }

    #[test]
    fn stage_materializes_changes_without_mutating_base() {
        let root = fixture_project("stage");
        let store = unique_temp_dir("store-stage");
        let revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_001".to_string(),
            project_root: root.clone(),
            candidate_store_root: store.clone(),
            changes: vec![
                CandidateFileChange::CreateOrReplace {
                    path: "RuntimeModule/src/lib.rs".to_string(),
                    bytes: b"pub fn candidate() {}".to_vec(),
                },
                CandidateFileChange::Delete {
                    path: "Input/input.none.json".to_string(),
                },
            ],
        })
        .unwrap();

        let candidate = PathBuf::from(&revision.candidate_root);
        assert_eq!(
            fs::read(candidate.join("RuntimeModule/src/lib.rs")).unwrap(),
            b"pub fn candidate() {}"
        );
        assert!(!candidate.join("Input/input.none.json").exists());
        assert!(root.join("Input/input.none.json").exists());
        assert_eq!(
            revision.changed_paths,
            vec!["Input/input.none.json", "RuntimeModule/src/lib.rs"]
        );
        assert_ne!(
            revision.base_project_digest,
            revision.candidate_project_digest
        );
        assert!(!candidate.join("Library").exists());
    }

    #[test]
    fn stage_reports_only_actual_byte_changes() {
        let root = fixture_project("actual-changes");
        let store = unique_temp_dir("store-actual-changes");
        let revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_actual_changes".to_string(),
            project_root: root,
            candidate_store_root: store,
            changes: vec![
                CandidateFileChange::CreateOrReplace {
                    path: "Scenes/Main.scene.json".to_string(),
                    bytes: b"scene".to_vec(),
                },
                CandidateFileChange::Delete {
                    path: "Input/missing.json".to_string(),
                },
            ],
        })
        .unwrap();

        assert!(revision.changed_paths.is_empty());
        assert_eq!(
            revision.base_project_digest,
            revision.candidate_project_digest
        );
    }

    #[test]
    fn schema_round_trip_preserves_request_revision_and_error() {
        let root = fixture_project("schema");
        let store = unique_temp_dir("store-schema");
        let request = CandidateProjectRevisionRequest {
            revision_id: "revision_schema".to_string(),
            project_root: root,
            candidate_store_root: store,
            changes: vec![CandidateFileChange::CreateOrReplace {
                path: "RuntimeModule/src/lib.rs".to_string(),
                bytes: b"schema".to_vec(),
            }],
        };
        let request_json = serde_json::to_string(&request).unwrap();
        let decoded_request: CandidateProjectRevisionRequest =
            serde_json::from_str(&request_json).unwrap();
        assert_eq!(decoded_request, request);

        let revision = CandidateProjectRevisionStore::stage(request).unwrap();
        let revision_json = serde_json::to_string(&revision).unwrap();
        let decoded_revision: CandidateProjectRevision =
            serde_json::from_str(&revision_json).unwrap();
        assert_eq!(decoded_revision, revision);

        let error = CandidateProjectRevisionError::new(
            "candidate_revision.test",
            "test diagnostic",
            None,
            "retry",
        );
        let error_json = serde_json::to_string(&error).unwrap();
        let decoded_error: CandidateProjectRevisionError =
            serde_json::from_str(&error_json).unwrap();
        assert_eq!(decoded_error, error);
    }

    #[test]
    fn verify_base_detects_drift() {
        let root = fixture_project("drift");
        let store = unique_temp_dir("store-drift");
        let revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_drift".to_string(),
            project_root: root.clone(),
            candidate_store_root: store,
            changes: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            CandidateProjectRevisionStore::verify_base(&revision, &root)
                .unwrap()
                .status,
            CandidateBaseVerificationStatus::Matched
        );
        fs::write(root.join("project.aife.json"), b"changed").unwrap();
        assert_eq!(
            CandidateProjectRevisionStore::verify_base(&revision, &root)
                .unwrap()
                .status,
            CandidateBaseVerificationStatus::Drifted
        );

        fs::write(
            Path::new(&revision.candidate_root).join("Scenes/Main.scene.json"),
            b"tampered candidate",
        )
        .unwrap();
        let error = CandidateProjectRevisionStore::verify_base(&revision, &root).unwrap_err();
        assert_eq!(error.code, "candidate_revision.candidate_digest_mismatch");
    }

    #[test]
    fn verify_base_rejects_tampered_changed_paths() {
        let root = fixture_project("changed-paths-tampering");
        let store = unique_temp_dir("store-changed-paths-tampering");
        let mut revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_changed_paths_tampering".to_string(),
            project_root: root.clone(),
            candidate_store_root: store,
            changes: vec![CandidateFileChange::CreateOrReplace {
                path: "RuntimeModule/src/lib.rs".to_string(),
                bytes: b"pub fn candidate() {}".to_vec(),
            }],
        })
        .unwrap();
        revision.changed_paths.clear();

        let error = CandidateProjectRevisionStore::verify_base(&revision, &root).unwrap_err();
        assert_eq!(error.code, "candidate_revision.changed_paths_mismatch");
    }

    #[test]
    fn stage_rejects_overlapping_store_and_cleans_failed_staging() {
        let root = fixture_project("overlap");
        let error = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_overlap".to_string(),
            project_root: root.clone(),
            candidate_store_root: root.join("Library/Candidates"),
            changes: Vec::new(),
        })
        .unwrap_err();
        assert_eq!(error.code, "candidate_revision.roots_overlap");
        assert!(!root.join("Library").exists());

        let store = unique_temp_dir("store-failed");
        let error = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_failed".to_string(),
            project_root: root,
            candidate_store_root: store.clone(),
            changes: vec![CandidateFileChange::CreateOrReplace {
                path: "../escape.rs".to_string(),
                bytes: Vec::new(),
            }],
        })
        .unwrap_err();
        assert_eq!(error.code, "candidate_revision.change_path_invalid");
        assert!(!store.join(".revision_failed.staging").exists());
        assert!(!store.join("revision_failed").exists());
    }

    #[test]
    fn discard_is_store_scoped_and_idempotent() {
        let root = fixture_project("discard");
        let store = unique_temp_dir("store-discard");
        let revision = CandidateProjectRevisionStore::stage(CandidateProjectRevisionRequest {
            revision_id: "revision_discard".to_string(),
            project_root: root,
            candidate_store_root: store.clone(),
            changes: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            CandidateProjectRevisionStore::discard(&revision, &store)
                .unwrap()
                .outcome,
            CandidateDiscardOutcome::Removed
        );
        assert_eq!(
            CandidateProjectRevisionStore::discard(&revision, &store)
                .unwrap()
                .outcome,
            CandidateDiscardOutcome::Missing
        );
    }

    #[cfg(windows)]
    #[test]
    fn source_junction_is_rejected() {
        use std::process::Command;

        let root = fixture_project("junction");
        let junction_target = unique_temp_dir("junction-target");
        fs::create_dir_all(&junction_target).unwrap();
        fs::write(junction_target.join("outside.txt"), b"outside").unwrap();
        let junction = root.join("LinkedSource");
        let status = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                junction_target.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let error = project_tree_digest(&root).unwrap_err();
        assert_eq!(error.code, "candidate_revision.source_link_rejected");
        fs::remove_dir(&junction).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_root_prefix_contains_descendants() {
        assert!(path_starts_with(
            Path::new(r"C:\project"),
            Path::new(r"C:\")
        ));
        assert!(!path_starts_with(
            Path::new(r"C:\project-other"),
            Path::new(r"C:\project")
        ));
        assert!(paths_equal(
            Path::new(r"\\?\C:\project"),
            Path::new(r"C:\project")
        ));
    }

    fn fixture_project(label: &str) -> PathBuf {
        let root = unique_temp_dir(label);
        fs::create_dir_all(root.join("Scenes")).unwrap();
        fs::create_dir_all(root.join("Input")).unwrap();
        fs::write(root.join("project.aife.json"), b"{}").unwrap();
        fs::write(root.join("Scenes/Main.scene.json"), b"scene").unwrap();
        fs::write(root.join("Input/input.none.json"), b"input").unwrap();
        root
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aife-candidate-{label}-{stamp}"))
    }

    fn in_memory_project_tree_digest_for_test(root: &Path) -> String {
        let files = project_source_files(root).unwrap();
        let mut payload = Vec::new();
        payload.extend_from_slice(PROJECT_TREE_DIGEST_SCHEMA_VERSION.as_bytes());
        payload.push(0);
        for file in files {
            let bytes = fs::read(&file.path).unwrap();
            payload.extend_from_slice(&(file.relative.len() as u64).to_le_bytes());
            payload.extend_from_slice(file.relative.as_bytes());
            payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            payload.extend_from_slice(&bytes);
        }
        let mut hasher = Sha256::new();
        hasher.update(payload);
        format!("sha256:{:x}", hasher.finalize())
    }
}
