use crate::mutation::mutation_error;
use crate::ContextError;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

pub(crate) struct ProjectAuthorityLock {
    file: File,
}

impl ProjectAuthorityLock {
    pub(crate) fn acquire(project_root: &Path, owner: &str) -> Result<Self, ContextError> {
        let authority_root = project_root.join(".aife/authoring");
        fs::create_dir_all(&authority_root).map_err(|error| {
            lock_error(
                "authoring_context.mutation_authority_unavailable",
                format!("Mutation authority directory cannot be created: {error}"),
                &authority_root,
            )
        })?;
        let path = authority_root.join("authority.lock");
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| {
                lock_error(
                    "authoring_context.mutation_authority_unavailable",
                    format!("Mutation authority lock carrier cannot be opened: {error}"),
                    &path,
                )
            })?;
        try_lock_exclusive(&file).map_err(|error| {
            lock_error(
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    "authoring_context.mutation_authority_busy"
                } else {
                    "authoring_context.mutation_authority_unavailable"
                },
                format!("Project mutation authority cannot be acquired: {error}"),
                &path,
            )
        })?;
        if let Err(error) = write_owner(&mut file, owner) {
            let _ = unlock(&file);
            return Err(lock_error(
                "authoring_context.mutation_authority_unavailable",
                format!("Mutation authority owner record cannot be written: {error}"),
                &path,
            ));
        }
        Ok(Self { file })
    }
}

impl Drop for ProjectAuthorityLock {
    fn drop(&mut self) {
        let _ = unlock(&self.file);
    }
}

fn write_owner(file: &mut File, owner: &str) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(file, "pid={} owner={owner}", std::process::id())?;
    file.flush()?;
    file.sync_all()
}

fn lock_error(code: &str, message: String, path: &Path) -> ContextError {
    mutation_error(
        code,
        message,
        Some(path.display().to_string()),
        "Wait for the current Engine mutation to finish, then refresh and retry.",
    )
}

#[cfg(windows)]
fn try_lock_exclusive(file: &File) -> std::io::Result<()> {
    use std::mem::zeroed;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let mut overlapped: OVERLAPPED = unsafe { zeroed() };
    let result = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if result == 0 {
        let error = std::io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(32 | 33)) {
            Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, error))
        } else {
            Err(error)
        }
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn unlock(file: &File) -> std::io::Result<()> {
    use std::mem::zeroed;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::UnlockFileEx;
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let mut overlapped: OVERLAPPED = unsafe { zeroed() };
    let result =
        unsafe { UnlockFileEx(file.as_raw_handle(), 0, u32::MAX, u32::MAX, &mut overlapped) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    let result = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Err(error)
        } else {
            Err(error)
        }
    }
}

#[cfg(unix)]
fn unlock(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_UN: i32 = 8;
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    if unsafe { flock(file.as_raw_fd(), LOCK_UN) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(windows, unix)))]
fn try_lock_exclusive(_file: &File) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "project mutation OS lock is unsupported on this platform",
    ))
}

#[cfg(not(any(windows, unix)))]
fn unlock(_file: &File) -> std::io::Result<()> {
    Ok(())
}
