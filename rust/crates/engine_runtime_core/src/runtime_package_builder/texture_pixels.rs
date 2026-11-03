//! Compiler input storage only. Published texture bytes and Runtime loaders are unchanged.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    io,
    ops::{Deref, DerefMut, Range},
    path::Path,
};

#[derive(Clone)]
pub struct RuntimePackageTexturePixels(Storage);

#[derive(Clone)]
enum Storage {
    Owned(Vec<u8>),
    #[cfg(windows)]
    Mapped {
        file: std::sync::Arc<ReadOnlyFile>,
        range: Range<usize>,
    },
}

impl RuntimePackageTexturePixels {
    /// The cache owner supplies the authenticated payload digest. Hold a read-only
    /// file lease and recheck bytes after opening, closing the lookup/open race.
    pub fn from_immutable_cache_file(path: &Path, digest: &str) -> io::Result<Self> {
        #[cfg(windows)]
        let value = {
            let file = std::sync::Arc::new(ReadOnlyFile::open(path)?);
            let len = file.len;
            Self(Storage::Mapped {
                file,
                range: 0..len,
            })
        };
        #[cfg(not(windows))]
        let value = Self::from(std::fs::read(path)?);
        if crate::canonical_digest::sha256_prefixed(&value) != digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "texture cache payload digest changed",
            ));
        }
        Ok(value)
    }

    pub fn into_range(self, range: Range<usize>) -> Option<Self> {
        self.as_slice().get(range.clone())?;
        match self.0 {
            Storage::Owned(bytes) => Some(bytes[range].to_vec().into()),
            #[cfg(windows)]
            Storage::Mapped {
                file,
                range: original,
            } => Some(Self(Storage::Mapped {
                file,
                range: original.start + range.start..original.start + range.end,
            })),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        match &self.0 {
            Storage::Owned(bytes) => bytes,
            #[cfg(windows)]
            Storage::Mapped { file, range } => &file.as_slice()[range.clone()],
        }
    }

    fn make_mut(&mut self) -> &mut Vec<u8> {
        if !matches!(self.0, Storage::Owned(_)) {
            self.0 = Storage::Owned(self.as_slice().to_vec());
        }
        match &mut self.0 {
            Storage::Owned(bytes) => bytes,
            #[cfg(windows)]
            Storage::Mapped { .. } => unreachable!(),
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        self.make_mut().pop()
    }
}
impl From<Vec<u8>> for RuntimePackageTexturePixels {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Storage::Owned(bytes))
    }
}
impl AsRef<[u8]> for RuntimePackageTexturePixels {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl Deref for RuntimePackageTexturePixels {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl DerefMut for RuntimePackageTexturePixels {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.make_mut()
    }
}
impl<T: AsRef<[u8]> + ?Sized> PartialEq<T> for RuntimePackageTexturePixels {
    fn eq(&self, other: &T) -> bool {
        self.as_slice() == other.as_ref()
    }
}
impl Eq for RuntimePackageTexturePixels {}
impl std::fmt::Debug for RuntimePackageTexturePixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimePackageTexturePixels")
            .field("len", &self.len())
            .finish()
    }
}
impl Serialize for RuntimePackageTexturePixels {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for RuntimePackageTexturePixels {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<u8>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(windows)]
struct ReadOnlyFile {
    // The open handle denies writes and deletion for the entire mapping lifetime.
    _file: std::fs::File,
    address: windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS,
    len: usize,
}
// SAFETY: Only a read view is exposed; the read-only file lease denies writing
// and deletion. Arc owns the mapping and unmaps only after the final reader drops.
#[cfg(windows)]
unsafe impl Send for ReadOnlyFile {}
#[cfg(windows)]
unsafe impl Sync for ReadOnlyFile {}

#[cfg(windows)]
impl ReadOnlyFile {
    fn open(path: &Path) -> io::Result<Self> {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_READ, PAGE_READONLY},
        };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)?;
        let len = usize::try_from(file.metadata()?.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "texture file is too large"))?;
        if len == 0 || len > isize::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid texture file length",
            ));
        }
        // SAFETY: file is an owned live handle; no security attributes or name.
        let mapping = unsafe {
            CreateFileMappingW(
                file.as_raw_handle(),
                std::ptr::null(),
                PAGE_READONLY,
                0,
                0,
                std::ptr::null(),
            )
        };
        if mapping.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Map the checked file length read-only. The view retains the
        // mapping object after its handle is closed; file stays owned by Self.
        let address = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, len) };
        let result = if address.Value.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self {
                _file: file,
                address,
                len,
            })
        };
        // SAFETY: This handle was returned by CreateFileMappingW exactly once.
        unsafe {
            CloseHandle(mapping);
        }
        result
    }
    fn as_slice(&self) -> &[u8] {
        // SAFETY: The non-null view covers len <= isize::MAX immutable bytes,
        // and self owns the file and view throughout the returned borrow.
        unsafe { std::slice::from_raw_parts(self.address.Value.cast::<u8>(), self.len) }
    }
}
#[cfg(windows)]
impl Drop for ReadOnlyFile {
    fn drop(&mut self) {
        // SAFETY: The view belongs to this last Arc owner; no borrows remain.
        unsafe {
            windows_sys::Win32::System::Memory::UnmapViewOfFile(self.address);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_texture_backing_preserves_bytes_digest_and_copy_on_write() {
        let path = std::env::temp_dir().join(format!(
            "371-texture-map-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bytes = (0..8192).map(|v| (v % 251) as u8).collect::<Vec<_>>();
        std::fs::write(&path, &bytes).unwrap();
        let digest = crate::canonical_digest::sha256_prefixed(&bytes);
        assert!(
            RuntimePackageTexturePixels::from_immutable_cache_file(&path, "sha256:bad").is_err()
        );
        let mapped =
            RuntimePackageTexturePixels::from_immutable_cache_file(&path, &digest).unwrap();
        assert_eq!(mapped, bytes);
        assert_eq!(
            serde_json::to_vec(&mapped).unwrap(),
            serde_json::to_vec(&bytes).unwrap()
        );
        let restored: RuntimePackageTexturePixels =
            serde_json::from_slice(&serde_json::to_vec(&mapped).unwrap()).unwrap();
        assert_eq!(restored, bytes);
        let mut part = mapped.clone().into_range(8..4096).unwrap();
        assert!(mapped.clone().into_range(8000..9000).is_none());
        let mut changed = mapped.clone();
        changed[8] ^= 255;
        assert_eq!(mapped[8], bytes[8]);
        assert_ne!(changed[8], mapped[8]);
        assert_eq!(part.pop(), Some(bytes[4095]));
        assert_eq!(part.len(), 4087);
        let lease = mapped.clone();
        #[cfg(windows)]
        {
            assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
            assert!(std::fs::remove_file(&path).is_err());
        }
        drop(mapped);
        assert_eq!(lease, bytes);
        #[cfg(windows)]
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        drop(lease);
        std::fs::write(&path, b"changed after final lease").unwrap();
        assert!(RuntimePackageTexturePixels::from_immutable_cache_file(&path, &digest).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
