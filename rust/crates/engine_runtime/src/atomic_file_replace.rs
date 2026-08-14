use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static REPLACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicReplaceStage {
    CreateParent,
    CreateTemp,
    WriteTemp,
    SyncTemp,
    StageExisting,
    Commit,
    Rollback,
    Cleanup,
}

impl AtomicReplaceStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateParent => "create_parent",
            Self::CreateTemp => "create_temp",
            Self::WriteTemp => "write_temp",
            Self::SyncTemp => "sync_temp",
            Self::StageExisting => "stage_existing",
            Self::Commit => "commit",
            Self::Rollback => "rollback",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Debug)]
pub struct AtomicReplaceError {
    pub path: PathBuf,
    pub stage: AtomicReplaceStage,
    pub source: io::Error,
    pub rollback_error: Option<io::Error>,
    pub next_action: &'static str,
}

impl std::fmt::Display for AtomicReplaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "atomic replace failed at {} for {}: {}",
            self.stage.as_str(),
            self.path.display(),
            self.source
        )?;
        if let Some(rollback_error) = &self.rollback_error {
            write!(formatter, "; rollback failed: {rollback_error}")?;
        }
        write!(formatter, "; next action: {}", self.next_action)
    }
}

impl std::error::Error for AtomicReplaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub fn atomic_file_replace(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), AtomicReplaceError> {
    atomic_file_replace_impl(path.as_ref(), bytes, FaultPoint::None)
}

fn atomic_file_replace_impl(
    path: &Path,
    bytes: &[u8],
    fault: FaultPoint,
) -> Result<(), AtomicReplaceError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|source| replace_error(path, AtomicReplaceStage::CreateParent, source))?;
    let (temp_path, backup_path) = sibling_paths(path);
    let mut staged_existing = false;

    let result = (|| {
        inject_fault(fault, FaultPoint::CreateTemp)
            .map_err(|source| replace_error(path, AtomicReplaceStage::CreateTemp, source))?;
        let mut temp = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| replace_error(path, AtomicReplaceStage::CreateTemp, source))?;
        inject_fault(fault, FaultPoint::WriteTemp)
            .map_err(|source| replace_error(path, AtomicReplaceStage::WriteTemp, source))?;
        temp.write_all(bytes)
            .map_err(|source| replace_error(path, AtomicReplaceStage::WriteTemp, source))?;
        temp.flush()
            .map_err(|source| replace_error(path, AtomicReplaceStage::WriteTemp, source))?;
        inject_fault(fault, FaultPoint::SyncTemp)
            .map_err(|source| replace_error(path, AtomicReplaceStage::SyncTemp, source))?;
        temp.sync_all()
            .map_err(|source| replace_error(path, AtomicReplaceStage::SyncTemp, source))?;
        drop(temp);

        if path.exists() {
            inject_fault(fault, FaultPoint::StageExisting)
                .map_err(|source| replace_error(path, AtomicReplaceStage::StageExisting, source))?;
            fs::rename(path, &backup_path)
                .map_err(|source| replace_error(path, AtomicReplaceStage::StageExisting, source))?;
            staged_existing = true;
        }

        inject_fault(fault, FaultPoint::Commit)
            .map_err(|source| replace_error(path, AtomicReplaceStage::Commit, source))?;
        if let Err(source) = fs::rename(&temp_path, path) {
            let rollback_error = rollback_existing(path, &backup_path, staged_existing).err();
            return Err(AtomicReplaceError {
                path: path.to_path_buf(),
                stage: AtomicReplaceStage::Commit,
                source,
                rollback_error,
                next_action: "Inspect the target directory and retry the save.",
            });
        }
        staged_existing = false;
        if backup_path.exists() {
            fs::remove_file(&backup_path)
                .map_err(|source| replace_error(path, AtomicReplaceStage::Cleanup, source))?;
        }
        Ok(())
    })();

    if result.is_err() && staged_existing {
        let rollback_error = rollback_existing(path, &backup_path, true).err();
        if let Err(mut error) = result {
            if error.rollback_error.is_none() {
                error.rollback_error = rollback_error;
            }
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    }
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_file(&backup_path);
    }
    result
}

fn rollback_existing(path: &Path, backup_path: &Path, staged_existing: bool) -> io::Result<()> {
    if !staged_existing || !backup_path.exists() {
        return Ok(());
    }
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(backup_path, path)
}

fn sibling_paths(path: &Path) -> (PathBuf, PathBuf) {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REPLACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let token = format!("{}-{stamp}-{sequence}", std::process::id());
    (
        path.with_file_name(format!(".{file_name}.{token}.tmp")),
        path.with_file_name(format!(".{file_name}.{token}.bak")),
    )
}

fn replace_error(path: &Path, stage: AtomicReplaceStage, source: io::Error) -> AtomicReplaceError {
    AtomicReplaceError {
        path: path.to_path_buf(),
        stage,
        source,
        rollback_error: None,
        next_action: "Keep the working copy dirty, inspect the path, and retry the save.",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultPoint {
    None,
    CreateTemp,
    WriteTemp,
    SyncTemp,
    StageExisting,
    Commit,
}

fn inject_fault(active: FaultPoint, current: FaultPoint) -> io::Result<()> {
    if active == current {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("injected {current:?} fault"),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_file_replace_commits_and_removes_temp_and_backup() {
        let root = test_root("success");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("scene.json");
        fs::write(&path, b"old").unwrap();
        atomic_file_replace(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn atomic_file_replace_faults_preserve_existing_file() {
        for fault in [
            FaultPoint::CreateTemp,
            FaultPoint::WriteTemp,
            FaultPoint::SyncTemp,
            FaultPoint::StageExisting,
            FaultPoint::Commit,
        ] {
            let root = test_root(&format!("fault-{fault:?}"));
            fs::create_dir_all(&root).unwrap();
            let path = root.join("asset.json");
            fs::write(&path, b"last-good").unwrap();
            let error = atomic_file_replace_impl(&path, b"new", fault).unwrap_err();
            assert_eq!(
                fs::read(&path).unwrap(),
                b"last-good",
                "fault={fault:?} error={error}"
            );
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1, "fault={fault:?}");
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let sequence = REPLACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "aife-atomic-replace-{label}-{}-{sequence}",
            std::process::id()
        ))
    }
}
