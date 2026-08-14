use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use std::fmt;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectRelativePath {
    path: PathBuf,
    display: String,
}

impl ProjectRelativePath {
    pub fn parse(path: impl AsRef<Path>) -> Result<Self, ProjectWriteError> {
        let raw = path.as_ref().to_string_lossy();
        if raw.is_empty() {
            return Err(ProjectWriteError::path("project_write.path_empty", None));
        }

        let normalized = raw.replace('\\', "/");
        if normalized.starts_with('/')
            || normalized.starts_with("//")
            || has_windows_drive_prefix(&normalized)
        {
            return Err(ProjectWriteError::path(
                "project_write.path_not_relative",
                Some(normalized),
            ));
        }
        if normalized.ends_with('/') || normalized.split('/').any(str::is_empty) {
            return Err(ProjectWriteError::path(
                "project_write.path_ambiguous",
                Some(normalized),
            ));
        }

        let parsed = Path::new(&normalized);
        for component in parsed.components() {
            match component {
                Component::Normal(_) => {}
                Component::ParentDir => {
                    return Err(ProjectWriteError::path(
                        "project_write.path_parent_component",
                        Some(normalized),
                    ));
                }
                Component::CurDir => {
                    return Err(ProjectWriteError::path(
                        "project_write.path_ambiguous",
                        Some(normalized),
                    ));
                }
                Component::Prefix(_) | Component::RootDir => {
                    return Err(ProjectWriteError::path(
                        "project_write.path_not_relative",
                        Some(normalized),
                    ));
                }
            }
        }

        Ok(Self {
            path: parsed.to_path_buf(),
            display: normalized,
        })
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }

    pub fn as_str(&self) -> &str {
        &self.display
    }
}

impl fmt::Display for ProjectRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display)
    }
}

impl TryFrom<&str> for ProjectRelativePath {
    type Error = ProjectWriteError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ProjectRelativePath {
    type Error = ProjectWriteError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectWriteOperation {
    OpenRoot,
    Read,
    CreateDirectory,
    AcquireLock,
    WriteAtomic,
    RemoveFile,
    PublishDirectory,
}

impl ProjectWriteOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenRoot => "open_root",
            Self::Read => "read",
            Self::CreateDirectory => "create_directory",
            Self::AcquireLock => "acquire_lock",
            Self::WriteAtomic => "write_atomic",
            Self::RemoveFile => "remove_file",
            Self::PublishDirectory => "publish_directory",
        }
    }
}

#[derive(Debug)]
pub struct ProjectWriteError {
    pub code: &'static str,
    pub operation: ProjectWriteOperation,
    pub relative_path: Option<String>,
    pub source: Option<io::Error>,
    pub rollback_error: Option<io::Error>,
}

impl ProjectWriteError {
    fn path(code: &'static str, relative_path: Option<String>) -> Self {
        Self {
            code,
            operation: ProjectWriteOperation::WriteAtomic,
            relative_path,
            source: None,
            rollback_error: None,
        }
    }

    fn io(
        code: &'static str,
        operation: ProjectWriteOperation,
        relative_path: Option<String>,
        source: io::Error,
    ) -> Self {
        Self {
            code,
            operation,
            relative_path,
            source: Some(source),
            rollback_error: None,
        }
    }

    fn with_rollback(mut self, rollback_error: Option<io::Error>) -> Self {
        self.rollback_error = rollback_error;
        if self.rollback_error.is_some() {
            self.code = "project_write.atomic_rollback_failed";
        }
        self
    }
}

impl fmt::Display for ProjectWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} during {}",
            self.code,
            self.operation.as_str()
        )?;
        if let Some(path) = &self.relative_path {
            write!(formatter, " for {path}")?;
        }
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        if let Some(rollback_error) = &self.rollback_error {
            write!(formatter, "; rollback failed: {rollback_error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProjectWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectWriteOutcome {
    Created,
    Replaced,
    Removed,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWriteReceipt {
    pub operation: ProjectWriteOperation,
    pub relative_path: String,
    pub outcome: ProjectWriteOutcome,
    pub bytes_written: u64,
}

#[derive(Clone)]
pub struct ProjectWriteScope {
    root: Arc<Dir>,
    display_root: Arc<PathBuf>,
}

#[derive(Clone)]
pub struct ProjectDirectoryWriter {
    scope: ProjectWriteScope,
}

#[derive(Debug)]
pub(crate) struct ProjectFileLock {
    scope: ProjectWriteScope,
    relative_path: ProjectRelativePath,
    held: bool,
}

impl ProjectFileLock {
    pub(crate) fn release(mut self) -> Result<ProjectWriteReceipt, ProjectWriteError> {
        let receipt = self.scope.remove_file(self.relative_path.as_path())?;
        self.held = false;
        Ok(receipt)
    }
}

impl Drop for ProjectFileLock {
    fn drop(&mut self) {
        if self.held {
            let _ = self.scope.remove_file(self.relative_path.as_path());
        }
    }
}

impl fmt::Debug for ProjectDirectoryWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectDirectoryWriter")
            .finish_non_exhaustive()
    }
}

impl ProjectDirectoryWriter {
    pub fn create_dir_all(&self, relative_path: impl AsRef<Path>) -> Result<(), ProjectWriteError> {
        self.scope.create_dir_all(relative_path)
    }

    pub fn write_atomic(
        &self,
        relative_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<ProjectWriteReceipt, ProjectWriteError> {
        self.scope.write_atomic(relative_path, bytes)
    }

    pub fn read(&self, relative_path: impl AsRef<Path>) -> Result<Vec<u8>, ProjectWriteError> {
        self.scope.read(relative_path)
    }
}

impl fmt::Debug for ProjectWriteScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectWriteScope")
            .field("display_root", &self.display_root)
            .finish_non_exhaustive()
    }
}

impl ProjectWriteScope {
    pub fn open(project_root: impl AsRef<Path>) -> Result<Self, ProjectWriteError> {
        let project_root = project_root.as_ref();
        let root = Dir::open_ambient_dir(project_root, ambient_authority()).map_err(|source| {
            ProjectWriteError::io(
                "project_write.root_unavailable",
                ProjectWriteOperation::OpenRoot,
                None,
                source,
            )
        })?;
        Ok(Self {
            root: Arc::new(root),
            display_root: Arc::new(project_root.to_path_buf()),
        })
    }

    pub fn display_root(&self) -> &Path {
        self.display_root.as_path()
    }

    pub fn read(&self, relative_path: impl AsRef<Path>) -> Result<Vec<u8>, ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        self.root.read(relative_path.as_path()).map_err(|source| {
            ProjectWriteError::io(
                "project_write.capability_denied",
                ProjectWriteOperation::Read,
                Some(relative_path.to_string()),
                source,
            )
        })
    }

    pub(crate) fn create_dir_all(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<(), ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        self.root
            .create_dir_all(relative_path.as_path())
            .map_err(|source| {
                ProjectWriteError::io(
                    "project_write.capability_denied",
                    ProjectWriteOperation::CreateDirectory,
                    Some(relative_path.to_string()),
                    source,
                )
            })
    }

    pub(crate) fn ensure_directory(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<(), ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        self.root
            .create_dir_all(relative_path.as_path())
            .and_then(|_| self.root.open_dir(relative_path.as_path()).map(drop))
            .map_err(|source| {
                ProjectWriteError::io(
                    "project_write.capability_denied",
                    ProjectWriteOperation::CreateDirectory,
                    Some(relative_path.to_string()),
                    source,
                )
            })
    }

    pub(crate) fn try_exists(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<bool, ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        self.root
            .try_exists(relative_path.as_path())
            .map_err(|source| {
                ProjectWriteError::io(
                    "project_write.capability_denied",
                    ProjectWriteOperation::Read,
                    Some(relative_path.to_string()),
                    source,
                )
            })
    }

    pub(crate) fn acquire_lock(
        &self,
        relative_path: impl AsRef<Path>,
        owner: &[u8],
    ) -> Result<ProjectFileLock, ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        let (parent, file_name) = self.open_parent(&relative_path, true)?;
        reject_final_link(
            &parent,
            &file_name,
            &relative_path,
            ProjectWriteOperation::AcquireLock,
        )?;
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        let mut file = parent.open_with(&file_name, &options).map_err(|source| {
            let code = if source.kind() == io::ErrorKind::AlreadyExists {
                "project_write.lock_held"
            } else {
                "project_write.lock_acquire_failed"
            };
            ProjectWriteError::io(
                code,
                ProjectWriteOperation::AcquireLock,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        file.write_all(owner)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|source| {
                let _ = parent.remove_file(&file_name);
                ProjectWriteError::io(
                    "project_write.lock_acquire_failed",
                    ProjectWriteOperation::AcquireLock,
                    Some(relative_path.to_string()),
                    source,
                )
            })?;
        Ok(ProjectFileLock {
            scope: self.clone(),
            relative_path,
            held: true,
        })
    }

    pub fn write_atomic(
        &self,
        relative_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<ProjectWriteReceipt, ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        let (parent, file_name) = self.open_parent(&relative_path, true)?;
        reject_final_link(
            &parent,
            &file_name,
            &relative_path,
            ProjectWriteOperation::WriteAtomic,
        )?;

        let existed = parent.try_exists(&file_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.capability_denied",
                ProjectWriteOperation::WriteAtomic,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        let (temp_name, backup_name) = sibling_names(&file_name);
        let mut staged_existing = false;

        let result = (|| {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            let mut temp = parent.open_with(&temp_name, &options).map_err(|source| {
                ProjectWriteError::io(
                    "project_write.atomic_create_temp_failed",
                    ProjectWriteOperation::WriteAtomic,
                    Some(relative_path.to_string()),
                    source,
                )
            })?;
            temp.write_all(bytes)
                .and_then(|_| temp.flush())
                .map_err(|source| {
                    ProjectWriteError::io(
                        "project_write.atomic_commit_failed",
                        ProjectWriteOperation::WriteAtomic,
                        Some(relative_path.to_string()),
                        source,
                    )
                })?;
            temp.sync_all().map_err(|source| {
                ProjectWriteError::io(
                    "project_write.atomic_commit_failed",
                    ProjectWriteOperation::WriteAtomic,
                    Some(relative_path.to_string()),
                    source,
                )
            })?;
            drop(temp);

            if existed {
                parent
                    .rename(&file_name, &parent, &backup_name)
                    .map_err(|source| {
                        ProjectWriteError::io(
                            "project_write.atomic_commit_failed",
                            ProjectWriteOperation::WriteAtomic,
                            Some(relative_path.to_string()),
                            source,
                        )
                    })?;
                staged_existing = true;
            }

            if let Err(source) = parent.rename(&temp_name, &parent, &file_name) {
                let rollback_error =
                    rollback_file(&parent, &file_name, &backup_name, staged_existing);
                staged_existing = false;
                return Err(ProjectWriteError::io(
                    "project_write.atomic_commit_failed",
                    ProjectWriteOperation::WriteAtomic,
                    Some(relative_path.to_string()),
                    source,
                )
                .with_rollback(rollback_error.err()));
            }
            staged_existing = false;

            if existed {
                if let Err(source) = parent.remove_file(&backup_name) {
                    let rollback_error = rollback_committed_file(&parent, &file_name, &backup_name);
                    return Err(ProjectWriteError::io(
                        "project_write.atomic_commit_failed",
                        ProjectWriteOperation::WriteAtomic,
                        Some(relative_path.to_string()),
                        source,
                    )
                    .with_rollback(rollback_error.err()));
                }
            }

            Ok(ProjectWriteReceipt {
                operation: ProjectWriteOperation::WriteAtomic,
                relative_path: relative_path.to_string(),
                outcome: if existed {
                    ProjectWriteOutcome::Replaced
                } else {
                    ProjectWriteOutcome::Created
                },
                bytes_written: bytes.len() as u64,
            })
        })();

        if result.is_err() {
            if staged_existing {
                let _ = rollback_file(&parent, &file_name, &backup_name, true);
            }
            let _ = parent.remove_file(&temp_name);
            if parent.try_exists(&file_name).unwrap_or(false) {
                let _ = parent.remove_file(&backup_name);
            }
        }
        result
    }

    pub fn remove_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<ProjectWriteReceipt, ProjectWriteError> {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        let (parent, file_name) = self.open_parent(&relative_path, false)?;
        reject_final_link(
            &parent,
            &file_name,
            &relative_path,
            ProjectWriteOperation::RemoveFile,
        )?;
        if !parent.try_exists(&file_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.capability_denied",
                ProjectWriteOperation::RemoveFile,
                Some(relative_path.to_string()),
                source,
            )
        })? {
            return Ok(ProjectWriteReceipt {
                operation: ProjectWriteOperation::RemoveFile,
                relative_path: relative_path.to_string(),
                outcome: ProjectWriteOutcome::Missing,
                bytes_written: 0,
            });
        }
        parent.remove_file(&file_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.remove_failed",
                ProjectWriteOperation::RemoveFile,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        Ok(ProjectWriteReceipt {
            operation: ProjectWriteOperation::RemoveFile,
            relative_path: relative_path.to_string(),
            outcome: ProjectWriteOutcome::Removed,
            bytes_written: 0,
        })
    }

    pub fn publish_directory_atomic<F>(
        &self,
        relative_path: impl AsRef<Path>,
        builder: F,
    ) -> Result<ProjectWriteReceipt, ProjectWriteError>
    where
        F: FnOnce(&ProjectDirectoryWriter) -> Result<(), ProjectWriteError>,
    {
        let relative_path = ProjectRelativePath::parse(relative_path)?;
        let (parent, directory_name) = self.open_parent(&relative_path, true)?;
        reject_final_directory_link(&parent, &directory_name, &relative_path)?;
        let existed = parent.try_exists(&directory_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.capability_denied",
                ProjectWriteOperation::PublishDirectory,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        let (staging_name, backup_name) = sibling_directory_names(&directory_name);
        parent.create_dir(&staging_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.publish_failed",
                ProjectWriteOperation::PublishDirectory,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        let staging = parent.open_dir(&staging_name).map_err(|source| {
            ProjectWriteError::io(
                "project_write.publish_failed",
                ProjectWriteOperation::PublishDirectory,
                Some(relative_path.to_string()),
                source,
            )
        })?;
        let writer = ProjectDirectoryWriter {
            scope: ProjectWriteScope {
                root: Arc::new(staging),
                display_root: Arc::new(self.display_root.join(relative_path.as_path())),
            },
        };

        if let Err(error) = builder(&writer) {
            drop(writer);
            let _ = parent.remove_dir_all(&staging_name);
            return Err(error);
        }
        drop(writer);

        let mut staged_existing = false;
        if existed {
            parent
                .rename(&directory_name, &parent, &backup_name)
                .map_err(|source| {
                    let _ = parent.remove_dir_all(&staging_name);
                    ProjectWriteError::io(
                        "project_write.publish_failed",
                        ProjectWriteOperation::PublishDirectory,
                        Some(relative_path.to_string()),
                        source,
                    )
                })?;
            staged_existing = true;
        }
        if let Err(source) = parent.rename(&staging_name, &parent, &directory_name) {
            let rollback_error = if staged_existing {
                parent.rename(&backup_name, &parent, &directory_name).err()
            } else {
                None
            };
            let _ = parent.remove_dir_all(&staging_name);
            return Err(ProjectWriteError::io(
                "project_write.publish_failed",
                ProjectWriteOperation::PublishDirectory,
                Some(relative_path.to_string()),
                source,
            )
            .with_rollback(rollback_error));
        }
        if staged_existing {
            parent.remove_dir_all(&backup_name).map_err(|source| {
                ProjectWriteError::io(
                    "project_write.publish_failed",
                    ProjectWriteOperation::PublishDirectory,
                    Some(relative_path.to_string()),
                    source,
                )
            })?;
        }
        Ok(ProjectWriteReceipt {
            operation: ProjectWriteOperation::PublishDirectory,
            relative_path: relative_path.to_string(),
            outcome: if existed {
                ProjectWriteOutcome::Replaced
            } else {
                ProjectWriteOutcome::Created
            },
            bytes_written: 0,
        })
    }

    fn open_parent(
        &self,
        relative_path: &ProjectRelativePath,
        create: bool,
    ) -> Result<(Dir, PathBuf), ProjectWriteError> {
        let parent_path = relative_path
            .as_path()
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let file_name = relative_path
            .as_path()
            .file_name()
            .expect("strict project relative paths have a final component")
            .into();
        if parent_path.as_os_str().is_empty() {
            return self
                .root
                .try_clone()
                .map(|parent| (parent, file_name))
                .map_err(|source| {
                    ProjectWriteError::io(
                        "project_write.capability_denied",
                        ProjectWriteOperation::WriteAtomic,
                        Some(relative_path.to_string()),
                        source,
                    )
                });
        }
        if create {
            self.root.create_dir_all(parent_path).map_err(|source| {
                ProjectWriteError::io(
                    "project_write.capability_denied",
                    ProjectWriteOperation::CreateDirectory,
                    Some(relative_path.to_string()),
                    source,
                )
            })?;
        }
        self.root
            .open_dir(parent_path)
            .map(|parent| (parent, file_name))
            .map_err(|source| {
                ProjectWriteError::io(
                    "project_write.capability_denied",
                    ProjectWriteOperation::WriteAtomic,
                    Some(relative_path.to_string()),
                    source,
                )
            })
    }
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn reject_final_link(
    parent: &Dir,
    file_name: &Path,
    relative_path: &ProjectRelativePath,
    operation: ProjectWriteOperation,
) -> Result<(), ProjectWriteError> {
    match parent.symlink_metadata(file_name) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ProjectWriteError {
            code: "project_write.final_link_rejected",
            operation,
            relative_path: Some(relative_path.to_string()),
            source: None,
            rollback_error: None,
        }),
        Ok(metadata) if !metadata.is_file() => Err(ProjectWriteError {
            code: "project_write.reparse_unsupported",
            operation,
            relative_path: Some(relative_path.to_string()),
            source: None,
            rollback_error: None,
        }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ProjectWriteError::io(
            "project_write.capability_denied",
            operation,
            Some(relative_path.to_string()),
            source,
        )),
    }
}

fn sibling_names(file_name: &Path) -> (PathBuf, PathBuf) {
    let file_name = file_name.to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let token = format!("{}-{stamp}-{sequence}", std::process::id());
    (
        PathBuf::from(format!(".{file_name}.{token}.tmp")),
        PathBuf::from(format!(".{file_name}.{token}.bak")),
    )
}

fn sibling_directory_names(directory_name: &Path) -> (PathBuf, PathBuf) {
    let directory_name = directory_name.to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let token = format!("{}-{stamp}-{sequence}", std::process::id());
    (
        PathBuf::from(format!(".{directory_name}.{token}.staging")),
        PathBuf::from(format!(".{directory_name}.{token}.backup")),
    )
}

fn reject_final_directory_link(
    parent: &Dir,
    directory_name: &Path,
    relative_path: &ProjectRelativePath,
) -> Result<(), ProjectWriteError> {
    match parent.symlink_metadata(directory_name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(ProjectWriteError {
                code: "project_write.final_link_rejected",
                operation: ProjectWriteOperation::PublishDirectory,
                relative_path: Some(relative_path.to_string()),
                source: None,
                rollback_error: None,
            })
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ProjectWriteError::io(
            "project_write.capability_denied",
            ProjectWriteOperation::PublishDirectory,
            Some(relative_path.to_string()),
            source,
        )),
    }
}

fn rollback_file(
    parent: &Dir,
    file_name: &Path,
    backup_name: &Path,
    staged: bool,
) -> io::Result<()> {
    if !staged {
        return Ok(());
    }
    parent.rename(backup_name, parent, file_name)
}

fn rollback_committed_file(parent: &Dir, file_name: &Path, backup_name: &Path) -> io::Result<()> {
    parent.remove_file(file_name)?;
    parent.rename(backup_name, parent, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn project_write_scope_rejects_unsafe_relative_paths() {
        for (path, code) in [
            ("", "project_write.path_empty"),
            ("/absolute", "project_write.path_not_relative"),
            ("C:/absolute", "project_write.path_not_relative"),
            ("../outside", "project_write.path_parent_component"),
            ("folder/../outside", "project_write.path_parent_component"),
            ("./file", "project_write.path_ambiguous"),
            ("folder//file", "project_write.path_ambiguous"),
            ("folder/", "project_write.path_ambiguous"),
        ] {
            let error = ProjectRelativePath::parse(path).unwrap_err();
            assert_eq!(error.code, code, "path={path}");
        }
        assert_eq!(
            ProjectRelativePath::parse("Scenes\\Main.scene.json")
                .unwrap()
                .as_str(),
            "Scenes/Main.scene.json"
        );
    }

    #[test]
    fn project_write_scope_creates_replaces_reads_and_removes() {
        let root = test_root("lifecycle");
        fs::create_dir_all(&root).unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();

        let created = scope.write_atomic("Scenes/Main.json", b"first").unwrap();
        assert_eq!(created.outcome, ProjectWriteOutcome::Created);
        assert_eq!(scope.read("Scenes/Main.json").unwrap(), b"first");

        let replaced = scope.write_atomic("Scenes/Main.json", b"second").unwrap();
        assert_eq!(replaced.outcome, ProjectWriteOutcome::Replaced);
        assert_eq!(scope.read("Scenes/Main.json").unwrap(), b"second");

        let removed = scope.remove_file("Scenes/Main.json").unwrap();
        assert_eq!(removed.outcome, ProjectWriteOutcome::Removed);
        assert!(!root.join("Scenes/Main.json").exists());
        assert_eq!(
            scope.remove_file("Scenes/Main.json").unwrap().outcome,
            ProjectWriteOutcome::Missing
        );
    }

    #[test]
    fn project_write_scope_lock_is_exclusive_and_releasable() {
        let root = test_root("exclusive-lock");
        fs::create_dir_all(&root).unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();

        let lock = scope
            .acquire_lock("Library/AssetPipeline/import.lock", b"transaction-a")
            .unwrap();
        let conflict = scope
            .acquire_lock("Library/AssetPipeline/import.lock", b"transaction-b")
            .unwrap_err();
        assert_eq!(conflict.code, "project_write.lock_held");

        lock.release().unwrap();
        let next = scope
            .acquire_lock("Library/AssetPipeline/import.lock", b"transaction-b")
            .unwrap();
        drop(next);
        assert!(!root.join("Library/AssetPipeline/import.lock").exists());
    }

    #[test]
    fn project_write_containment_atomic_replace_breaks_external_hard_link_alias() {
        let root = test_root("hard-link");
        let outside = test_root("hard-link-outside");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("sentinel.bin");
        fs::write(&outside_file, b"outside-sentinel").unwrap();
        fs::hard_link(&outside_file, root.join("Assets/linked.bin")).unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();

        scope
            .write_atomic("Assets/linked.bin", b"project-data")
            .unwrap();

        assert_eq!(fs::read(&outside_file).unwrap(), b"outside-sentinel");
        assert_eq!(
            fs::read(root.join("Assets/linked.bin")).unwrap(),
            b"project-data"
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_write_containment_rejects_final_symlink_write_and_remove() {
        use std::os::unix::fs::symlink;

        let root = test_root("final-symlink");
        let outside = test_root("final-symlink-outside");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("sentinel.bin");
        fs::write(&outside_file, b"outside-sentinel").unwrap();
        symlink(&outside_file, root.join("Assets/linked.bin")).unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();

        assert_eq!(
            scope
                .write_atomic("Assets/linked.bin", b"project-data")
                .unwrap_err()
                .code,
            "project_write.final_link_rejected"
        );
        assert_eq!(
            scope.remove_file("Assets/linked.bin").unwrap_err().code,
            "project_write.final_link_rejected"
        );
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside-sentinel");
    }

    #[test]
    fn cloned_scope_reuses_open_root_capability() {
        let root = test_root("clone");
        fs::create_dir_all(&root).unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();
        let cloned = scope.clone();

        cloned.write_atomic("Settings/value.json", b"{}").unwrap();

        assert_eq!(scope.read("Settings/value.json").unwrap(), b"{}");
    }

    #[test]
    fn project_write_scope_publishes_directory_atomically() {
        let root = test_root("publish-directory");
        fs::create_dir_all(root.join("Build/output")).unwrap();
        fs::write(root.join("Build/output/stale.bin"), b"stale").unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();

        let receipt = scope
            .publish_directory_atomic("Build/output", |writer| {
                writer.write_atomic("data/value.bin", b"current")?;
                Ok(())
            })
            .unwrap();

        assert_eq!(receipt.outcome, ProjectWriteOutcome::Replaced);
        assert_eq!(
            fs::read(root.join("Build/output/data/value.bin")).unwrap(),
            b"current"
        );
        assert!(!root.join("Build/output/stale.bin").exists());
        assert_eq!(fs::read_dir(root.join("Build")).unwrap().count(), 1);
    }

    fn test_root(label: &str) -> PathBuf {
        let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aife-project-write-{label}-{}-{sequence}-{nonce}",
            std::process::id(),
        ))
    }
}
