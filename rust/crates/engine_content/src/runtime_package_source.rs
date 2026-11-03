//! Read-only platform I/O. Package schemas and decoding stay in their existing owners.
use crate::runtime_package_path::{
    safe_join_runtime_package, RuntimePackagePath, RuntimePackagePathClaims,
};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RuntimePackageSource {
    root: PathBuf,
    storage: PackageStorage,
}

#[derive(Debug, Clone)]
enum PackageStorage {
    Directory,
    Memory(Arc<BTreeMap<String, Arc<[u8]>>>),
}

impl RuntimePackageSource {
    pub fn directory(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            storage: PackageStorage::Directory,
        }
    }

    /// Caller verifies the delivery manifest before constructing this immutable source.
    pub fn memory(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> io::Result<Self> {
        let mut claims = RuntimePackagePathClaims::default();
        let mut entries = BTreeMap::new();
        for (key, bytes) in files {
            let path = RuntimePackagePath::parse(key).map_err(invalid_path)?;
            claims.claim(&path).map_err(invalid_path)?;
            entries.insert(path.windows_collision_key(), Arc::from(bytes));
        }
        Ok(Self {
            root: PathBuf::from("runtime-package"),
            storage: PackageStorage::Memory(Arc::new(entries)),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn is_memory(&self) -> bool {
        matches!(self.storage, PackageStorage::Memory(_))
    }

    pub fn resolve(&self, relative: &str) -> io::Result<PathBuf> {
        match &self.storage {
            PackageStorage::Directory => {
                safe_join_runtime_package(&self.root, relative).map_err(invalid_path)
            }
            PackageStorage::Memory(_) => {
                let path = RuntimePackagePath::parse(relative).map_err(invalid_path)?;
                Ok(self.root.join(path.as_str()))
            }
        }
    }

    pub fn read(&self, relative: &str) -> io::Result<Vec<u8>> {
        self.read_limited(relative, usize::MAX)
    }

    pub fn read_limited(&self, relative: &str, limit: usize) -> io::Result<Vec<u8>> {
        let path = self.resolve(relative)?;
        match &self.storage {
            PackageStorage::Directory => {
                let file = std::fs::File::open(path)?;
                let length = file.metadata()?.len();
                if length > limit as u64 {
                    return Err(too_large(relative));
                }
                // Preserve exact payload backing as std::fs::read did. Growing an
                // empty Vec through Take can double power-of-two texel buffers
                // just to discover EOF, and consumers retain that spare capacity.
                let mut bytes = Vec::with_capacity(length as usize);
                file.take((limit as u64).saturating_add(1))
                    .read_to_end(&mut bytes)?;
                if bytes.len() > limit {
                    return Err(too_large(relative));
                }
                Ok(bytes)
            }
            PackageStorage::Memory(files) => {
                let bytes = files.get(&relative.to_ascii_lowercase()).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("package file missing: {relative}"),
                    )
                })?;
                if bytes.len() > limit {
                    return Err(too_large(relative));
                }
                Ok(bytes.to_vec())
            }
        }
    }

    /// Adapts existing parser paths without treating a logical path as an OS file.
    pub fn read_path(&self, path: &Path) -> io::Result<Vec<u8>> {
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path is outside package source",
            )
        })?;
        let relative = relative
            .to_str()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "package path is not UTF-8")
            })?
            .replace('\\', "/");
        self.read(&relative)
    }

    pub fn read_string_path(&self, path: &Path) -> io::Result<String> {
        String::from_utf8(self.read_path(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

fn invalid_path(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}
fn too_large(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("package file exceeds size limit: {path}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_payload_capacity_does_not_double_at_eof() {
        let root = std::env::temp_dir().join(format!("371-source-capacity-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let expected = vec![0x71; 1024 * 1024];
        std::fs::write(root.join("pixels.rgba8"), &expected).unwrap();
        let source = RuntimePackageSource::directory(&root);
        let bytes = source.read("pixels.rgba8").unwrap();
        std::fs::remove_file(root.join("pixels.rgba8")).unwrap();
        std::fs::remove_dir(root).unwrap();
        assert_eq!(bytes, expected);
        assert_eq!(
            bytes.capacity(),
            bytes.len(),
            "Retained package payload must not keep EOF growth slack"
        );
    }

    #[test]
    fn directory_limited_reads_preserve_exact_limit_and_oversize_refusal() {
        let root = std::env::temp_dir().join(format!("371-source-limit-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("pixels.rgba8"), [0x71; 4096]).unwrap();
        std::fs::write(root.join("empty"), []).unwrap();
        let source = RuntimePackageSource::directory(&root);
        let exact = source.read_limited("pixels.rgba8", 4096).unwrap();
        let oversized = source.read_limited("pixels.rgba8", 4095).unwrap_err();
        let empty = source.read_limited("empty", 0).unwrap();
        std::fs::remove_file(root.join("pixels.rgba8")).unwrap();
        std::fs::remove_file(root.join("empty")).unwrap();
        std::fs::remove_dir(root).unwrap();
        assert_eq!(exact, [0x71; 4096]);
        assert_eq!(exact.capacity(), exact.len());
        assert_eq!(oversized.kind(), io::ErrorKind::InvalidData);
        assert!(empty.is_empty());
        assert_eq!(empty.capacity(), 0);
    }

    #[test]
    fn memory_rejects_paths_and_collisions() {
        for path in [
            "../escape",
            "/absolute",
            "a//b",
            "C:/file",
            "a%2f../b",
            "a\\b",
        ] {
            // Percent escapes are literal package keys, never decoded by this source.
            if path.contains('%') {
                continue;
            }
            assert!(
                RuntimePackageSource::memory([(path.into(), vec![])]).is_err(),
                "{path}"
            );
        }
        assert!(RuntimePackageSource::memory([
            ("A.json".into(), vec![]),
            ("a.json".into(), vec![])
        ])
        .is_err());
    }
    #[test]
    fn memory_uses_the_existing_unambiguous_package_collision_key() {
        let source =
            RuntimePackageSource::memory([("AUI/aui-manifest.json".into(), b"{}".to_vec())])
                .unwrap();
        assert_eq!(source.read("aui/aui-manifest.json").unwrap(), b"{}");
        assert_eq!(source.read("AUI/aui-manifest.json").unwrap(), b"{}");
    }
    #[test]
    fn immutable_source_reads_and_bounds_files() {
        let source =
            RuntimePackageSource::memory([("scenes/main.json".into(), b"{}".to_vec())]).unwrap();
        assert_eq!(
            source
                .read_path(&source.resolve("scenes/main.json").unwrap())
                .unwrap(),
            b"{}"
        );
        assert_eq!(
            source.read("missing").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(source.read("../escape").is_err());
        assert!(source.read_path(Path::new("elsewhere/file")).is_err());
        assert!(source.read_limited("scenes/main.json", 1).is_err());
        assert_eq!(source.clone().read("scenes/main.json").unwrap(), b"{}");
    }
    #[test]
    fn memory_and_directory_return_identical_bytes() {
        let root = std::env::temp_dir().join(format!("375-source-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("manifest.json"), b"{\"valid\":true}").unwrap();
        let memory =
            RuntimePackageSource::memory([("manifest.json".into(), b"{\"valid\":true}".to_vec())])
                .unwrap();
        let directory = RuntimePackageSource::directory(&root);
        assert_eq!(
            memory.read("manifest.json").unwrap(),
            directory.read("manifest.json").unwrap()
        );
        assert!(directory.read("../escape").is_err());
        std::fs::remove_file(root.join("manifest.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
