use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static PUBLISH_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const RENAME_RETRY_TIMEOUT: Duration = Duration::from_secs(5);
const RENAME_RETRY_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicDirectoryPublishError {
    pub code: &'static str,
    pub path: PathBuf,
    pub message: String,
    pub next_action: &'static str,
}

impl std::fmt::Display for AtomicDirectoryPublishError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} for {}: {}; next action: {}",
            self.code,
            self.path.display(),
            self.message,
            self.next_action
        )
    }
}

impl std::error::Error for AtomicDirectoryPublishError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicDirectoryPublishFault {
    None,
    AfterStagingWrite,
    BeforePublishRename,
    AfterPublishRename,
}

#[derive(Debug)]
pub struct AtomicDirectoryPublishGuard {
    file: File,
}

impl AtomicDirectoryPublishGuard {
    pub fn acquire(final_dir: &Path) -> Result<Self, AtomicDirectoryPublishError> {
        let parent = final_dir.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            publish_error(
                "output_publish_parent_failed",
                parent,
                format!("failed to create output parent: {error}"),
            )
        })?;
        let name = final_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("published-directory");
        let path = parent.join(format!(".{name}.publish.lock"));
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                publish_error(
                    "output_publish_lock_open_failed",
                    &path,
                    format!("failed to open publish lock: {error}"),
                )
            })?;
        file.try_lock()
            .map_err(|error| AtomicDirectoryPublishError {
                code: "output_publish_busy",
                path: path.clone(),
                message: format!("another publisher owns the output lock: {error}"),
                next_action: "Retry after the active directory publish finishes.",
            })?;
        Ok(Self { file })
    }
}

impl Drop for AtomicDirectoryPublishGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn atomic_directory_publish<W, V>(
    final_dir: &Path,
    write_staging: W,
    validate: V,
) -> Result<(), AtomicDirectoryPublishError>
where
    W: FnOnce(&Path) -> Result<(), String>,
    V: Fn(&Path) -> Result<(), String>,
{
    atomic_directory_publish_with_fault(
        final_dir,
        AtomicDirectoryPublishFault::None,
        write_staging,
        validate,
    )
}

pub fn atomic_directory_publish_with_fault<W, V>(
    final_dir: &Path,
    fault: AtomicDirectoryPublishFault,
    write_staging: W,
    validate: V,
) -> Result<(), AtomicDirectoryPublishError>
where
    W: FnOnce(&Path) -> Result<(), String>,
    V: Fn(&Path) -> Result<(), String>,
{
    let _guard = AtomicDirectoryPublishGuard::acquire(final_dir)?;
    let parent = final_dir.parent().unwrap_or(Path::new("."));
    let name = final_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("published-directory");
    let backup_dir = parent.join(format!(".{name}.backup"));
    recover_publish_orphan(final_dir, &backup_dir, &validate)?;

    let sequence = PUBLISH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging_dir = parent.join(format!(".{name}.staging-{}-{sequence}", std::process::id()));
    fs::create_dir(&staging_dir).map_err(|error| {
        publish_error(
            "output_staging_create_failed",
            &staging_dir,
            format!("failed to create unique staging directory: {error}"),
        )
    })?;

    let attempt = (|| {
        write_staging(&staging_dir).map_err(|message| {
            publish_error("output_staging_write_failed", &staging_dir, message)
        })?;
        if fault == AtomicDirectoryPublishFault::AfterStagingWrite {
            return Err(publish_error(
                "output_staging_injected_failure",
                &staging_dir,
                "injected failure after staging write".to_string(),
            ));
        }
        validate(&staging_dir).map_err(|message| {
            publish_error("output_staging_preload_failed", &staging_dir, message)
        })?;

        let had_final = final_dir.exists();
        if had_final {
            rename_with_retry(final_dir, &backup_dir).map_err(|error| {
                publish_error(
                    "output_publish_backup_failed",
                    final_dir,
                    format!("failed to stage last-good backup: {error}"),
                )
            })?;
        }
        if fault == AtomicDirectoryPublishFault::BeforePublishRename {
            restore_last_good(final_dir, &backup_dir, had_final)?;
            return Err(publish_error(
                "output_publish_injected_failure",
                final_dir,
                "injected failure before staging publish rename".to_string(),
            ));
        }
        if let Err(error) = rename_with_retry(&staging_dir, final_dir) {
            restore_last_good(final_dir, &backup_dir, had_final)?;
            return Err(publish_error(
                "output_publish_rename_failed",
                final_dir,
                format!("failed to publish staged directory: {error}"),
            ));
        }
        if fault == AtomicDirectoryPublishFault::AfterPublishRename {
            restore_last_good(final_dir, &backup_dir, had_final)?;
            return Err(publish_error(
                "output_publish_postload_failed",
                final_dir,
                "injected failure after publish rename".to_string(),
            ));
        }
        if let Err(message) = validate(final_dir) {
            restore_last_good(final_dir, &backup_dir, had_final)?;
            return Err(publish_error(
                "output_publish_postload_failed",
                final_dir,
                message,
            ));
        }
        if backup_dir.exists() {
            remove_owned_publish_dir(parent, &backup_dir, &format!(".{name}.backup"))?;
        }
        Ok(())
    })();

    if attempt.is_err() && staging_dir.exists() {
        let _ = remove_owned_publish_dir(parent, &staging_dir, &format!(".{name}.staging-"));
    }
    attempt
}

fn recover_publish_orphan<V>(
    final_dir: &Path,
    backup_dir: &Path,
    validate: &V,
) -> Result<(), AtomicDirectoryPublishError>
where
    V: Fn(&Path) -> Result<(), String>,
{
    if !backup_dir.exists() {
        return Ok(());
    }
    if !final_dir.exists() {
        return rename_with_retry(backup_dir, final_dir).map_err(|error| {
            publish_error(
                "output_publish_orphan_restore_failed",
                backup_dir,
                format!("failed to restore orphaned backup: {error}"),
            )
        });
    }
    if validate(final_dir).is_ok() {
        let parent = final_dir.parent().unwrap_or(Path::new("."));
        let name = backup_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(".published-directory.backup");
        return remove_owned_publish_dir(parent, backup_dir, name);
    }
    fs::remove_dir_all(final_dir).map_err(|error| {
        publish_error(
            "output_publish_orphan_invalid_final_cleanup_failed",
            final_dir,
            format!("failed to remove invalid orphan final: {error}"),
        )
    })?;
    rename_with_retry(backup_dir, final_dir).map_err(|error| {
        publish_error(
            "output_publish_orphan_restore_failed",
            backup_dir,
            format!("failed to restore last-good backup: {error}"),
        )
    })
}

fn restore_last_good(
    final_dir: &Path,
    backup_dir: &Path,
    had_final: bool,
) -> Result<(), AtomicDirectoryPublishError> {
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(|error| AtomicDirectoryPublishError {
            code: "output_publish_rollback_failed",
            path: final_dir.to_path_buf(),
            message: format!("failed to remove failed published directory: {error}"),
            next_action: "Preserve final and backup paths for manual recovery.",
        })?;
    }
    if had_final {
        rename_with_retry(backup_dir, final_dir).map_err(|error| AtomicDirectoryPublishError {
            code: "output_publish_rollback_failed",
            path: backup_dir.to_path_buf(),
            message: format!("failed to restore last-good directory backup: {error}"),
            next_action: "Preserve final and backup paths for manual recovery.",
        })?;
    }
    Ok(())
}

fn rename_with_retry(source: &Path, destination: &Path) -> io::Result<()> {
    let started = Instant::now();
    loop {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error)
                if is_transient_rename_error(&error)
                    && started.elapsed() < RENAME_RETRY_TIMEOUT =>
            {
                thread::sleep(RENAME_RETRY_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient_rename_error(error: &io::Error) -> bool {
    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(5 | 32 | 33))
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
}

fn remove_owned_publish_dir(
    parent: &Path,
    path: &Path,
    required_prefix: &str,
) -> Result<(), AtomicDirectoryPublishError> {
    if path.parent() != Some(parent)
        || !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(required_prefix))
    {
        return Err(publish_error(
            "output_publish_cleanup_scope_violation",
            path,
            "refused to clean a path outside the owned publish scope".to_string(),
        ));
    }
    fs::remove_dir_all(path).map_err(|error| {
        publish_error(
            "output_publish_cleanup_failed",
            path,
            format!("failed to clean owned publish directory: {error}"),
        )
    })
}

fn publish_error(code: &'static str, path: &Path, message: String) -> AtomicDirectoryPublishError {
    AtomicDirectoryPublishError {
        code,
        path: path.to_path_buf(),
        message,
        next_action:
            "Inspect the structured publish stage and retry without deleting last-good output.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn atomic_directory_publish_replaces_stale_payload_and_restores_last_good_on_faults() {
        let root = temp_root("replace-rollback");
        let final_dir = root.join("release");
        atomic_directory_publish(
            &final_dir,
            |staging| {
                fs::write(staging.join("payload.txt"), b"last-good")
                    .map_err(|error| error.to_string())
            },
            validate_fixture,
        )
        .unwrap();
        fs::write(final_dir.join("stale.txt"), b"stale").unwrap();
        atomic_directory_publish(
            &final_dir,
            |staging| {
                fs::write(staging.join("payload.txt"), b"current")
                    .map_err(|error| error.to_string())
            },
            validate_fixture,
        )
        .unwrap();
        assert_eq!(fs::read(final_dir.join("payload.txt")).unwrap(), b"current");
        assert!(!final_dir.join("stale.txt").exists());

        for fault in [
            AtomicDirectoryPublishFault::AfterStagingWrite,
            AtomicDirectoryPublishFault::BeforePublishRename,
            AtomicDirectoryPublishFault::AfterPublishRename,
        ] {
            atomic_directory_publish_with_fault(
                &final_dir,
                fault,
                |staging| {
                    fs::write(staging.join("payload.txt"), b"faulted")
                        .map_err(|error| error.to_string())
                },
                validate_fixture,
            )
            .unwrap_err();
            assert_eq!(fs::read(final_dir.join("payload.txt")).unwrap(), b"current");
        }
    }

    #[test]
    fn atomic_directory_publish_guard_is_single_writer_with_hard_timeout() {
        let root = temp_root("single-writer");
        fs::create_dir_all(&root).unwrap();
        let final_dir = root.join("release");
        let first = AtomicDirectoryPublishGuard::acquire(&final_dir).unwrap();
        let competing_dir = final_dir.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender
                .send(AtomicDirectoryPublishGuard::acquire(&competing_dir))
                .unwrap();
        });
        let error = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("competing publisher must return before hard timeout")
            .unwrap_err();
        assert_eq!(error.code, "output_publish_busy");
        drop(first);
        worker.join().unwrap();
        AtomicDirectoryPublishGuard::acquire(&final_dir).unwrap();
        assert!(root.join(".release.publish.lock").is_file());
    }

    fn validate_fixture(path: &Path) -> Result<(), String> {
        let payload = fs::read(path.join("payload.txt")).map_err(|error| error.to_string())?;
        if payload.is_empty() {
            Err("payload is empty".to_string())
        } else {
            Ok(())
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("atomic-directory-publish-{name}-{stamp}"))
    }
}
