//! Host-owned opaque bytes. Project keys are never filesystem paths.
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorKind {
    InvalidKey,
    Busy,
    CapabilityGap,
    NotFound,
    TooLarge,
    Io,
}

#[derive(Debug)]
pub struct StorageError {
    pub kind: StorageErrorKind,
    pub source: io::Error,
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.source)
    }
}
impl std::error::Error for StorageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitState {
    NotCommitted,
    Committed,
    Unknown,
}
#[derive(Debug)]
pub struct StorageCommit {
    pub state: CommitState,
    pub error: Option<StorageError>,
}

fn failure(kind: StorageErrorKind, message: &str) -> StorageError {
    StorageError {
        kind,
        source: io::Error::other(message.to_owned()),
    }
}
fn io_failure(source: io::Error) -> StorageError {
    StorageError {
        kind: StorageErrorKind::Io,
        source,
    }
}

fn key_name(key: &str) -> Result<String, StorageError> {
    if key.is_empty()
        || key.len() > 256
        || key
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.ends_with(['.', ' ']))
        || key
            .chars()
            .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
    {
        return Err(failure(
            StorageErrorKind::InvalidKey,
            "expected controlled relative key",
        ));
    }
    Ok(format!("{:x}.bytes", Sha256::digest(key.as_bytes())))
}

fn namespace(identity: &str, profile: &str) -> Result<String, StorageError> {
    if identity.is_empty() || profile.is_empty() || identity.len() > 4096 || profile.len() > 4096 {
        return Err(failure(
            StorageErrorKind::InvalidKey,
            "host identity/profile must be nonempty and bounded",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(b"project-storage.v1\0");
    for value in [identity, profile] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Debug)]
pub struct ProjectStorage {
    #[cfg(test)]
    root: PathBuf,
    #[cfg(windows)]
    platform: windows::Owner,
}

impl ProjectStorage {
    /// `base` and stable identity/profile come from the trusted host, never package hashes or DLL input.
    /// Test hosts must inject their own isolated base. No fallback user directory is selected here.
    pub fn open(base: &Path, identity: &str, profile: &str) -> Result<Self, StorageError> {
        let name = namespace(identity, profile)?;
        #[cfg(windows)]
        {
            let (_root, platform) = windows::Owner::open(base, &name)?;
            Ok(Self {
                #[cfg(test)]
                root: _root,
                platform,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (base, name);
            Err(failure(
                StorageErrorKind::CapabilityGap,
                "controlled storage is not qualified on this platform",
            ))
        }
    }
    pub fn read(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>, StorageError> {
        let name = key_name(key)?;
        #[cfg(windows)]
        {
            self.platform.read(&name, max_bytes)
        }
        #[cfg(not(windows))]
        {
            let _ = (name, max_bytes);
            Err(failure(
                StorageErrorKind::CapabilityGap,
                "unsupported platform",
            ))
        }
    }
    pub fn write(&mut self, key: &str, bytes: &[u8]) -> StorageCommit {
        self.write_observed(key, bytes, |_| Ok(()))
    }
    fn write_observed(
        &mut self,
        key: &str,
        bytes: &[u8],
        observer: impl FnMut(WritePoint) -> io::Result<()>,
    ) -> StorageCommit {
        let name = match key_name(key) {
            Ok(name) => name,
            Err(error) => {
                return StorageCommit {
                    state: CommitState::NotCommitted,
                    error: Some(error),
                }
            }
        };
        #[cfg(windows)]
        {
            self.platform.write(&name, bytes, observer)
        }
        #[cfg(not(windows))]
        {
            let _ = (name, bytes, observer);
            StorageCommit {
                state: CommitState::NotCommitted,
                error: Some(failure(
                    StorageErrorKind::CapabilityGap,
                    "unsupported platform",
                )),
            }
        }
    }
    pub fn delete(&mut self, key: &str) -> StorageCommit {
        let name = match key_name(key) {
            Ok(name) => name,
            Err(error) => {
                return StorageCommit {
                    state: CommitState::NotCommitted,
                    error: Some(error),
                }
            }
        };
        #[cfg(windows)]
        {
            self.platform.delete(&name)
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            StorageCommit {
                state: CommitState::NotCommitted,
                error: Some(failure(
                    StorageErrorKind::CapabilityGap,
                    "unsupported platform",
                )),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WritePoint {
    BeforeWrite,
    BeforeSync,
    BeforeReplace,
    AfterReplace,
}

#[cfg(all(test, windows))]
mod tests;
#[cfg(windows)]
mod windows;
