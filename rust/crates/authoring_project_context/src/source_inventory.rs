use crate::{ContextError, DiagnosticStage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

pub const SOURCE_POLICY_VERSION: &str = "authoring-source-policy.v1";
const SOURCE_INVENTORY_SCHEMA_VERSION: &str = "authoring-source-inventory.v1";
const LEGACY_PROJECT_TREE_DIGEST_SCHEMA_VERSION: &str = "project-tree-digest.v1";
const DIGEST_IO_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceInventoryEntry {
    pub relative_path: String,
    pub source_kind: String,
    pub length: u64,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceInventory {
    project_root: PathBuf,
    entries: Vec<SourceInventoryEntry>,
    source_digest: String,
}

impl CanonicalSourceInventory {
    pub fn capture(project_root: impl AsRef<Path>) -> Result<Self, ContextError> {
        Self::capture_cancellable(project_root, || false)
    }

    pub fn capture_cancellable(
        project_root: impl AsRef<Path>,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<Self, ContextError> {
        Self::capture_cancellable_with_hook(project_root, is_cancelled, || {})
    }

    fn capture_cancellable_with_hook(
        project_root: impl AsRef<Path>,
        is_cancelled: impl Fn() -> bool,
        between_scans: impl FnOnce(),
    ) -> Result<Self, ContextError> {
        let requested_root = project_root.as_ref();
        reject_link_or_reparse(requested_root, DiagnosticStage::Refresh)?;
        let project_root = requested_root.canonicalize().map_err(|error| {
            ContextError::new(
                "authoring_context.root_unavailable",
                format!("Project root cannot be canonicalized: {error}"),
                DiagnosticStage::Refresh,
                Some(requested_root.display().to_string()),
                "Choose an existing project directory and retry.",
            )
        })?;
        if !project_root.is_dir() {
            return Err(ContextError::new(
                "authoring_context.root_not_directory",
                "Project root is not a directory.",
                DiagnosticStage::Refresh,
                Some(project_root.display().to_string()),
                "Choose a project directory and retry.",
            ));
        }

        let first = scan_project(&project_root, &is_cancelled)?;
        between_scans();
        let second = scan_project(&project_root, &is_cancelled)?;
        if first != second {
            return Err(source_changed_error(&project_root));
        }
        let source_digest = source_digest(&second);
        Ok(Self {
            project_root,
            entries: second,
            source_digest,
        })
    }

    #[cfg(test)]
    pub(crate) fn capture_with_between_scan_hook(
        project_root: impl AsRef<Path>,
        between_scans: impl FnOnce(),
    ) -> Result<Self, ContextError> {
        Self::capture_cancellable_with_hook(project_root, || false, between_scans)
    }

    pub fn entries(&self) -> &[SourceInventoryEntry] {
        &self.entries
    }

    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub(crate) fn entry(&self, relative_path: &str) -> Option<&SourceInventoryEntry> {
        self.entries
            .binary_search_by(|entry| entry.relative_path.as_str().cmp(relative_path))
            .ok()
            .map(|index| &self.entries[index])
    }

    pub(crate) fn read_verified(
        &self,
        entry: &SourceInventoryEntry,
    ) -> Result<Vec<u8>, ContextError> {
        read_verified_cancellable(&self.project_root, entry, &|| false)
    }

    fn confirm_unchanged(&self, is_cancelled: &impl Fn() -> bool) -> Result<(), ContextError> {
        if scan_project(&self.project_root, is_cancelled)? != self.entries {
            return Err(source_changed_error(&self.project_root));
        }
        Ok(())
    }
}

pub struct CanonicalSourcePolicy;

impl CanonicalSourcePolicy {
    pub fn allows_change(project_root: &Path, relative_path: &Path) -> bool {
        if canonical_relative(relative_path).is_err() || excludes_root_path(relative_path) {
            return false;
        }

        let mut ancestor = project_root.to_path_buf();
        let mut inside_cargo_tree = false;
        for component in relative_path.components() {
            let Component::Normal(value) = component else {
                return false;
            };
            inside_cargo_tree = inside_cargo_tree || directory_is_cargo_root(&ancestor);
            if value.to_string_lossy().eq_ignore_ascii_case("target") && inside_cargo_tree {
                return false;
            }
            ancestor.push(value);
        }
        true
    }
}

pub fn legacy_project_digest(project_root: impl AsRef<Path>) -> Result<String, ContextError> {
    legacy_project_digest_cancellable(project_root, || false)
}

pub fn legacy_project_digest_cancellable(
    project_root: impl AsRef<Path>,
    is_cancelled: impl Fn() -> bool,
) -> Result<String, ContextError> {
    let inventory = CanonicalSourceInventory::capture_cancellable(project_root, &is_cancelled)?;
    let mut hasher = Sha256::new();
    hasher.update(LEGACY_PROJECT_TREE_DIGEST_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    for entry in inventory.entries() {
        reject_cancelled(inventory.project_root(), &is_cancelled)?;
        let bytes = read_verified_cancellable(inventory.project_root(), entry, &is_cancelled)?;
        hasher.update((entry.relative_path.len() as u64).to_le_bytes());
        hasher.update(entry.relative_path.as_bytes());
        hasher.update(entry.length.to_le_bytes());
        hasher.update(bytes);
    }
    inventory.confirm_unchanged(&is_cancelled)?;
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn scan_project(
    project_root: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<Vec<SourceInventoryEntry>, ContextError> {
    let mut entries = Vec::new();
    let mut portable_paths = BTreeMap::new();
    collect_project_entries(
        project_root,
        project_root,
        false,
        &mut portable_paths,
        &mut entries,
        is_cancelled,
    )?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(entries)
}

fn collect_project_entries(
    project_root: &Path,
    directory: &Path,
    inside_cargo_tree: bool,
    portable_paths: &mut BTreeMap<String, String>,
    entries: &mut Vec<SourceInventoryEntry>,
    is_cancelled: &impl Fn() -> bool,
) -> Result<(), ContextError> {
    reject_cancelled(directory, is_cancelled)?;
    let mut directory_entries = fs::read_dir(directory)
        .map_err(|error| {
            ContextError::new(
                "authoring_context.directory_read_failed",
                format!("Project directory cannot be read: {error}"),
                DiagnosticStage::Refresh,
                Some(directory.display().to_string()),
                "Restore readable project directories and retry.",
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ContextError::new(
                "authoring_context.directory_entry_failed",
                format!("Project directory entry cannot be read: {error}"),
                DiagnosticStage::Refresh,
                Some(directory.display().to_string()),
                "Restore readable project directories and retry.",
            )
        })?;
    directory_entries.sort_by_key(|entry| entry.file_name());
    let inside_cargo_tree = inside_cargo_tree || directory_is_cargo_root(directory);

    for directory_entry in directory_entries {
        reject_cancelled(directory, is_cancelled)?;
        let path = directory_entry.path();
        let relative = path.strip_prefix(project_root).map_err(|_| {
            ContextError::new(
                "authoring_context.source_escaped_root",
                "Project source escaped the canonical project root.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Resolve the project containment violation.",
            )
        })?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ContextError::new(
                "authoring_context.source_metadata_failed",
                format!("Project source metadata cannot be read: {error}"),
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Restore a regular project source tree and retry.",
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ContextError::new(
                "authoring_context.source_link_rejected",
                "Project authority does not follow symbolic links, junctions or reparse points.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Replace the link with a project-owned regular file or directory.",
            ));
        }

        if metadata.is_dir() {
            if excludes_directory(relative, &path, inside_cargo_tree) {
                continue;
            }
            register_portable_path(relative, portable_paths)?;
            collect_project_entries(
                project_root,
                &path,
                inside_cargo_tree,
                portable_paths,
                entries,
                is_cancelled,
            )?;
        } else if metadata.is_file() {
            if excludes_root_path(relative) {
                continue;
            }
            let relative_path = register_portable_path(relative, portable_paths)?;
            let content_digest = hash_file(&path, metadata.len(), is_cancelled)?;
            entries.push(SourceInventoryEntry {
                relative_path,
                source_kind: "file".to_string(),
                length: metadata.len(),
                content_digest,
            });
        } else {
            return Err(ContextError::new(
                "authoring_context.source_type_rejected",
                "Project source contains an unsupported filesystem entry.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Remove the unsupported filesystem entry.",
            ));
        }
    }
    Ok(())
}

fn register_portable_path(
    relative: &Path,
    portable_paths: &mut BTreeMap<String, String>,
) -> Result<String, ContextError> {
    let portable = canonical_relative(relative)?;
    let folded = portable.to_lowercase();
    if let Some(existing) = portable_paths.get(&folded) {
        if existing != &portable {
            return Err(ContextError::new(
                "authoring_context.path_case_collision",
                format!("Project paths collide after case folding: '{existing}' and '{portable}'."),
                DiagnosticStage::Refresh,
                Some(portable),
                "Rename one path so the project is portable across case-insensitive filesystems.",
            ));
        }
    } else {
        portable_paths.insert(folded, portable.clone());
    }
    Ok(portable)
}

fn canonical_relative(path: &Path) -> Result<String, ContextError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(ContextError::new(
                "authoring_context.relative_path_invalid",
                "Project source path is not canonical relative syntax.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Normalize the project source path and retry.",
            ));
        };
        let value = value.to_str().ok_or_else(|| {
            ContextError::new(
                "authoring_context.non_utf8_path_rejected",
                "Project source contains a non-UTF-8 path.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Rename the path using valid UTF-8 characters.",
            )
        })?;
        if value.is_empty() || value.contains(['/', '\\']) || value.ends_with([' ', '.']) {
            return Err(ContextError::new(
                "authoring_context.non_portable_path_rejected",
                "Project source contains a path component that is not portable.",
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Rename the path using portable project-relative syntax.",
            ));
        }
        parts.push(value);
    }
    if parts.is_empty() {
        return Err(ContextError::new(
            "authoring_context.relative_path_invalid",
            "Project source path must identify a project-relative entry.",
            DiagnosticStage::Refresh,
            Some(path.display().to_string()),
            "Provide a project-relative source path.",
        ));
    }
    Ok(parts.join("/"))
}

fn hash_file(
    path: &Path,
    expected_length: u64,
    is_cancelled: &impl Fn() -> bool,
) -> Result<String, ContextError> {
    let source = fs::File::open(path).map_err(|error| {
        ContextError::new(
            "authoring_context.file_read_failed",
            format!("Project source file cannot be opened: {error}"),
            DiagnosticStage::Refresh,
            Some(path.display().to_string()),
            "Restore a readable regular source file and retry.",
        )
    })?;
    let mut reader = BufReader::with_capacity(DIGEST_IO_BUFFER_BYTES, source);
    let mut buffer = [0_u8; DIGEST_IO_BUFFER_BYTES];
    let mut consumed = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        reject_cancelled(path, is_cancelled)?;
        let read = reader.read(&mut buffer).map_err(|error| {
            ContextError::new(
                "authoring_context.file_read_failed",
                format!("Project source file cannot be read: {error}"),
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Restore a readable regular source file and retry.",
            )
        })?;
        if read == 0 {
            break;
        }
        consumed = consumed.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    let final_length = fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| {
            ContextError::new(
                "authoring_context.file_metadata_failed",
                format!("Project source metadata cannot be re-read: {error}"),
                DiagnosticStage::Refresh,
                Some(path.display().to_string()),
                "Retry after project source writes have completed.",
            )
        })?;
    if consumed != expected_length || final_length != expected_length {
        return Err(source_changed_error(path));
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn read_verified_cancellable(
    project_root: &Path,
    entry: &SourceInventoryEntry,
    is_cancelled: &impl Fn() -> bool,
) -> Result<Vec<u8>, ContextError> {
    let mut path = project_root.to_path_buf();
    for component in Path::new(&entry.relative_path).components() {
        let Component::Normal(value) = component else {
            return Err(source_changed_error(project_root));
        };
        path.push(value);
        reject_link_or_reparse(&path, DiagnosticStage::Refresh)?;
    }
    reject_cancelled(&path, is_cancelled)?;
    let bytes = fs::read(&path).map_err(|error| {
        ContextError::new(
            "authoring_context.file_read_failed",
            format!("Project source file cannot be read: {error}"),
            DiagnosticStage::Refresh,
            Some(path.display().to_string()),
            "Restore the source file and retry from a fresh revision.",
        )
    })?;
    let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    if bytes.len() as u64 != entry.length || digest != entry.content_digest {
        return Err(source_changed_error(&path));
    }
    Ok(bytes)
}

fn source_digest(entries: &[SourceInventoryEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_INVENTORY_SCHEMA_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(SOURCE_POLICY_VERSION.as_bytes());
    hasher.update([0]);
    for entry in entries {
        hasher.update((entry.relative_path.len() as u64).to_le_bytes());
        hasher.update(entry.relative_path.as_bytes());
        hasher.update((entry.source_kind.len() as u64).to_le_bytes());
        hasher.update(entry.source_kind.as_bytes());
        hasher.update(entry.length.to_le_bytes());
        hasher.update((entry.content_digest.len() as u64).to_le_bytes());
        hasher.update(entry.content_digest.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn excludes_root_path(relative: &Path) -> bool {
    let mut components = relative.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    if first.to_string_lossy().eq_ignore_ascii_case("RuntimeModule")
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

fn excludes_directory(relative: &Path, path: &Path, inside_cargo_tree: bool) -> bool {
    excludes_root_path(relative)
        || (inside_cargo_tree
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("target")))
}

fn directory_is_cargo_root(directory: &Path) -> bool {
    fs::symlink_metadata(directory.join("Cargo.toml"))
        .ok()
        .is_some_and(|metadata| metadata.is_file() && !is_link_or_reparse(&metadata))
}

fn reject_cancelled(path: &Path, is_cancelled: &impl Fn() -> bool) -> Result<(), ContextError> {
    if is_cancelled() {
        return Err(ContextError::new(
            "authoring_context.digest_cancelled",
            "Project source digest was cancelled.",
            DiagnosticStage::Refresh,
            Some(path.display().to_string()),
            "Retry when project inspection can continue.",
        ));
    }
    Ok(())
}

fn source_changed_error(path: &Path) -> ContextError {
    ContextError::new(
        "authoring_context.source_changed_during_refresh",
        "Project source changed while its canonical inventory was being captured.",
        DiagnosticStage::Refresh,
        Some(path.display().to_string()),
        "Retry after project source writes have completed.",
    )
}

fn reject_link_or_reparse(path: &Path, stage: DiagnosticStage) -> Result<(), ContextError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ContextError::new(
            "authoring_context.source_metadata_failed",
            format!("Project source metadata cannot be read: {error}"),
            stage,
            Some(path.display().to_string()),
            "Restore a regular project source tree and retry.",
        )
    })?;
    if is_link_or_reparse(&metadata) {
        return Err(ContextError::new(
            "authoring_context.source_link_rejected",
            "Project authority does not follow symbolic links, junctions or reparse points.",
            stage,
            Some(path.display().to_string()),
            "Replace the link with a project-owned regular file or directory.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_case_fold_collision_is_rejected() {
        let mut portable_paths = BTreeMap::new();
        register_portable_path(Path::new("Scenes/Main.scene.json"), &mut portable_paths).unwrap();

        let error =
            register_portable_path(Path::new("scenes/main.scene.json"), &mut portable_paths)
                .unwrap_err();

        assert_eq!(
            error.diagnostic.code,
            "authoring_context.path_case_collision"
        );
    }
}
