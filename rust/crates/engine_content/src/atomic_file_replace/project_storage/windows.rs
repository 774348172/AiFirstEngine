use super::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::{
    fs::{MetadataExt, OpenOptionsExt},
    io::{AsRawHandle, FromRawHandle},
};
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Wdk::{Foundation::OBJECT_ATTRIBUTES, Storage::FileSystem as nt};
use windows_sys::Win32::{Foundation::*, Storage::FileSystem::*, System::IO::IO_STATUS_BLOCK};

const DRIVE_FIXED: u32 = 3;
const OBJ_CASE_INSENSITIVE: u32 = 0x40;
const FILE_RENAME_REPLACE_IF_EXISTS: u32 = 1;
const FILE_RENAME_POSIX_SEMANTICS: u32 = 2;

#[derive(Debug)]
pub(super) struct Owner {
    directories: Vec<File>,
    _lease: File,
}
impl Owner {
    pub fn open(base: &Path, namespace: &str) -> Result<(PathBuf, Self), StorageError> {
        Self::open_observed(base, namespace, |_| Ok(()))
    }
    pub(super) fn open_observed(
        base: &Path,
        namespace: &str,
        after_base_open: impl FnOnce(&File) -> io::Result<()>,
    ) -> Result<(PathBuf, Self), StorageError> {
        use std::path::{Component, Prefix};
        let mut parts = base.components();
        let Some(Component::Prefix(prefix)) = parts.next() else {
            return Err(gap("expected absolute local drive root"));
        };
        if !matches!(prefix.kind(), Prefix::Disk(_)) || parts.next() != Some(Component::RootDir) {
            return Err(gap(
                "UNC, device, verbatim and drive-relative roots are unsupported",
            ));
        }
        let mut path = PathBuf::from(prefix.as_os_str());
        path.push("\\");
        let volume: Vec<u16> = path
            .to_str()
            .unwrap()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: volume is a live null-terminated local drive root.
        if unsafe { GetDriveTypeW(volume.as_ptr()) } != DRIVE_FIXED {
            return Err(gap("only fixed local NTFS storage is qualified"));
        }
        // Only the trusted drive root uses a path-based open. Every subsequent component,
        // leaf, enumeration and rename operates relative to a still-open verified handle.
        let root = OpenOptions::new()
            .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)
            .map_err(io_failure)?;
        check_directory(&root)?;
        let mut directories = vec![root];
        for part in parts {
            let Component::Normal(part) = part else {
                return Err(gap("storage root contains traversal"));
            };
            let name = part
                .to_str()
                .ok_or_else(|| gap("storage root must be valid Unicode"))?;
            if name.ends_with(['.', ' ']) || name.contains(':') {
                return Err(gap("ambiguous root component"));
            }
            let directory = relative_open(
                directories.last().unwrap(),
                name,
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                nt::FILE_OPEN_IF,
                nt::FILE_DIRECTORY_FILE,
            )?;
            check_directory(&directory)?;
            path.push(part);
            directories.push(directory);
        }
        let mut filesystem = [0u16; 32];
        // SAFETY: live directory handle and bounded writable filesystem buffer.
        let ok = unsafe {
            GetVolumeInformationByHandleW(
                directories.last().unwrap().as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        };
        if ok == 0
            || String::from_utf16_lossy(
                &filesystem[..filesystem
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(filesystem.len())],
            ) != "NTFS"
        {
            return Err(gap("expected qualified NTFS namespace semantics"));
        }
        after_base_open(directories.last().unwrap()).map_err(io_failure)?;
        let directory = relative_open(
            directories.last().unwrap(),
            namespace,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            nt::FILE_OPEN_IF,
            nt::FILE_DIRECTORY_FILE,
        )?;
        check_directory(&directory)?;
        path.push(namespace);
        directories.push(directory);
        let lease = relative_open(
            directories.last().unwrap(),
            ".aife-lease",
            GENERIC_READ | GENERIC_WRITE,
            0,
            nt::FILE_OPEN_IF,
            nt::FILE_NON_DIRECTORY_FILE,
        )
        .map_err(|mut e| {
            if matches!(e.source.raw_os_error(), Some(32 | 33)) {
                e.kind = StorageErrorKind::Busy;
            }
            e
        })?;
        regular_file(&lease)?;
        let owner = Self {
            directories,
            _lease: lease,
        };
        for name in directory_names(owner.directory())? {
            if owned_temp(&name) {
                let file = owner.leaf(&name, DELETE | FILE_READ_ATTRIBUTES, 0)?;
                disposition(&file).map_err(io_failure)?;
            }
        }
        Ok((path, owner))
    }
    fn directory(&self) -> &File {
        self.directories.last().unwrap()
    }
    fn leaf(&self, name: &str, access: u32, share: u32) -> Result<File, StorageError> {
        let file = relative_open(
            self.directory(),
            name,
            access,
            share,
            nt::FILE_OPEN,
            nt::FILE_NON_DIRECTORY_FILE,
        )?;
        regular_file(&file)?;
        Ok(file)
    }
    pub fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>, StorageError> {
        let mut file = self.leaf(name, GENERIC_READ, FILE_SHARE_READ)?;
        let length = file.metadata().map_err(io_failure)?.len();
        if length > limit as u64 {
            return Err(failure(
                StorageErrorKind::TooLarge,
                "read exceeds caller reservation",
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length as usize)
            .map_err(|_| failure(StorageErrorKind::TooLarge, "read allocation unavailable"))?;
        bytes.resize(length as usize, 0);
        file.read_exact(&mut bytes).map_err(io_failure)?;
        Ok(bytes)
    }
    pub fn write(
        &mut self,
        name: &str,
        bytes: &[u8],
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
    ) -> StorageCommit {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let temporary = format!(
            ".aife-tmp-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let mut file = match relative_open(
            self.directory(),
            &temporary,
            GENERIC_WRITE | DELETE | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ,
            nt::FILE_CREATE,
            nt::FILE_NON_DIRECTORY_FILE,
        ) {
            Ok(file) => file,
            Err(error) => return not_committed(error),
        };
        let prepared = (|| {
            observer(WritePoint::BeforeWrite)?;
            file.write_all(bytes)?;
            file.flush()?;
            observer(WritePoint::BeforeSync)?;
            file.sync_all()?;
            observer(WritePoint::BeforeReplace)?;
            Ok::<(), io::Error>(())
        })();
        if let Err(source) = prepared {
            let _ = disposition(&file);
            return not_committed(io_failure(source));
        }
        match rename(&file, self.directory(), name) {
            Ok(()) => StorageCommit {
                state: CommitState::Committed,
                error: observer(WritePoint::AfterReplace).err().map(io_failure),
            },
            Err(source) => {
                let state = rename_failure_state(&source);
                // A candidate might already be the target after an ambiguous error.
                if state == CommitState::NotCommitted {
                    let _ = disposition(&file);
                }
                StorageCommit {
                    state,
                    error: Some(io_failure(source)),
                }
            }
        }
    }
    pub fn delete(&mut self, name: &str) -> StorageCommit {
        let file = match self.leaf(name, DELETE | FILE_READ_ATTRIBUTES, 0) {
            Ok(file) => file,
            Err(error) => return not_committed(error),
        };
        match disposition(&file) {
            Ok(()) => StorageCommit {
                state: CommitState::Committed,
                error: None,
            },
            Err(source) => StorageCommit {
                state: rename_failure_state(&source),
                error: Some(io_failure(source)),
            },
        }
    }
}
fn gap(message: &str) -> StorageError {
    failure(StorageErrorKind::CapabilityGap, message)
}
fn not_committed(error: StorageError) -> StorageCommit {
    StorageCommit {
        state: CommitState::NotCommitted,
        error: Some(error),
    }
}
fn nt_result(status: NTSTATUS) -> io::Result<()> {
    if status >= 0 {
        Ok(())
    } else {
        // SAFETY: pure status conversion accepts every NTSTATUS value.
        Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ))
    }
}
fn relative_open(
    parent: &File,
    name: &str,
    access: u32,
    share: u32,
    disposition: u32,
    options: u32,
) -> Result<File, StorageError> {
    if name.is_empty() || name.contains(['/', '\\', ':', '\0']) || matches!(name, "." | "..") {
        return Err(gap("native open requires one controlled component"));
    }
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    let length = u16::try_from(wide.len() * 2).map_err(|_| gap("native component too long"))?;
    let object_name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle = std::ptr::null_mut();
    let mut iosb = std::mem::MaybeUninit::<IO_STATUS_BLOCK>::uninit();
    // SAFETY: live root, counted single-component name and valid output pointers.
    // Synchronous OPEN_REPARSE_POINT opens the leaf itself, with no unchecked prefix.
    let status = unsafe {
        nt::NtCreateFile(
            &mut handle,
            access | SYNCHRONIZE,
            &attributes,
            iosb.as_mut_ptr(),
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            share,
            disposition,
            options | nt::FILE_OPEN_REPARSE_POINT | nt::FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    nt_result(status).map_err(|source| StorageError {
        kind: if source.raw_os_error() == Some(1921) {
            StorageErrorKind::CapabilityGap
        } else if source.kind() == io::ErrorKind::NotFound {
            StorageErrorKind::NotFound
        } else {
            StorageErrorKind::Io
        },
        source,
    })?;
    // SAFETY: successful NtCreateFile transfers one newly owned handle to File.
    Ok(unsafe { File::from_raw_handle(handle) })
}
fn check_directory(file: &File) -> Result<(), StorageError> {
    let metadata = file.metadata().map_err(io_failure)?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(gap("reparse/non-directory storage root"));
    }
    Ok(())
}
fn regular_file(file: &File) -> Result<(), StorageError> {
    let metadata = file.metadata().map_err(io_failure)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(gap("reparse/non-file storage leaf"));
    }
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: live handle and writable descriptor initialized on success.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io_failure(io::Error::last_os_error()));
    }
    // SAFETY: successful API call above initialized info.
    if unsafe { info.assume_init() }.nNumberOfLinks != 1 {
        return Err(gap("hard-linked storage leaf"));
    }
    Ok(())
}
fn directory_names(directory: &File) -> Result<Vec<String>, StorageError> {
    let mut result = Vec::new();
    let mut buffer = [0u64; 1024];
    loop {
        // SAFETY: live enumeratable directory and aligned 8192-byte output buffer.
        if unsafe {
            GetFileInformationByHandleEx(
                directory.as_raw_handle(),
                FileFullDirectoryInfo,
                buffer.as_mut_ptr().cast(),
                std::mem::size_of_val(&buffer) as u32,
            )
        } == 0
        {
            let source = io::Error::last_os_error();
            if source.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                return Ok(result);
            }
            return Err(io_failure(source));
        }
        let mut offset = 0usize;
        loop {
            let header = std::mem::offset_of!(FILE_FULL_DIR_INFO, FileName);
            if offset + std::mem::size_of::<FILE_FULL_DIR_INFO>() > std::mem::size_of_val(&buffer) {
                return Err(gap("invalid directory header"));
            }
            // SAFETY: OS reply has an aligned header at this bounded offset.
            let info = unsafe {
                &*buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<FILE_FULL_DIR_INFO>()
            };
            let length = info.FileNameLength as usize;
            if !length.is_multiple_of(2)
                || offset + header + length > std::mem::size_of_val(&buffer)
            {
                return Err(gap("invalid directory name"));
            }
            // SAFETY: validated UTF16 name extent in the OS-filled reply.
            let name = unsafe {
                std::slice::from_raw_parts(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset + header)
                        .cast::<u16>(),
                    length / 2,
                )
            };
            result.push(String::from_utf16_lossy(name));
            if info.NextEntryOffset == 0 {
                break;
            }
            let next = info.NextEntryOffset as usize;
            if next < header || !next.is_multiple_of(8) {
                return Err(gap("invalid directory offset"));
            }
            offset += next;
        }
    }
}
fn disposition(file: &File) -> io::Result<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: 1 };
    // SAFETY: live file and fixed-layout descriptor; acts on this exact handle.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn rename(file: &File, parent: &File, name: &str) -> io::Result<()> {
    let name: Vec<u16> = name.encode_utf16().collect();
    let offset = std::mem::offset_of!(nt::FILE_RENAME_INFORMATION, FileName);
    let size = (offset + name.len() * 2).max(std::mem::size_of::<nt::FILE_RENAME_INFORMATION>());
    let mut buffer = vec![0u64; size.div_ceil(8)];
    let info = buffer.as_mut_ptr().cast::<nt::FILE_RENAME_INFORMATION>();
    let mut iosb = std::mem::MaybeUninit::<IO_STATUS_BLOCK>::uninit();
    // SAFETY: aligned bounded header/name backing; live source DELETE handle and exact
    // destination directory. One synchronous POSIX replacement, never stage the old file.
    unsafe {
        (*info).Anonymous.Flags = FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS;
        (*info).RootDirectory = parent.as_raw_handle();
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            name.len(),
        );
        nt_result(nt::NtSetInformationFile(
            file.as_raw_handle(),
            iosb.as_mut_ptr(),
            info.cast(),
            size as u32,
            nt::FileRenameInformationEx,
        ))
    }
}
pub(super) fn rename_failure_state(error: &io::Error) -> CommitState {
    match error.raw_os_error() {
        Some(2 | 3 | 5 | 17 | 32 | 33 | 50 | 80 | 87 | 123 | 183 | 206) => {
            CommitState::NotCommitted
        }
        _ => CommitState::Unknown,
    }
}
fn owned_temp(name: &str) -> bool {
    name.strip_prefix(".aife-tmp-")
        .and_then(|s| s.strip_suffix(".tmp"))
        .is_some_and(|s| {
            s.split_once('-').is_some_and(|(pid, sequence)| {
                !pid.is_empty()
                    && !sequence.is_empty()
                    && pid.bytes().all(|b| b.is_ascii_digit())
                    && sequence.bytes().all(|b| b.is_ascii_digit())
            })
        })
}
