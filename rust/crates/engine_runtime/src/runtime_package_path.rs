use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePackagePath(String);

impl RuntimePackagePath {
    pub fn parse(value: impl Into<String>) -> Result<Self, RuntimePackagePathError> {
        let value = value.into();
        validate_relative_package_path(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn windows_collision_key(&self) -> String {
        self.0
            .split('/')
            .map(|segment| segment.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePackagePathError {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for RuntimePackagePathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {} ({})", self.code, self.message, self.path)
    }
}

impl std::error::Error for RuntimePackagePathError {}

#[derive(Debug, Default)]
pub struct RuntimePackagePathClaims {
    by_windows_key: BTreeMap<String, String>,
}

impl RuntimePackagePathClaims {
    pub fn claim(&mut self, path: &RuntimePackagePath) -> Result<(), RuntimePackagePathError> {
        let key = path.windows_collision_key();
        if let Some(existing) = self.by_windows_key.get(&key) {
            return Err(RuntimePackagePathError {
                code: "runtime_package_path_collision",
                path: path.as_str().to_string(),
                message: format!(
                    "package path collides on Windows with previously claimed path {existing}"
                ),
            });
        }
        self.by_windows_key.insert(key, path.as_str().to_string());
        Ok(())
    }
}

pub fn validate_package_path_segment(segment: &str) -> Result<(), RuntimePackagePathError> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(path_error(
            "runtime_package_path_invalid_segment",
            segment,
            "path segment must be non-empty and cannot be dot or dot-dot",
        ));
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(path_error(
            "runtime_package_path_windows_trim_collision",
            segment,
            "Windows path segments cannot end in a dot or space",
        ));
    }
    if segment.chars().any(|character| {
        character.is_control()
            || matches!(character, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*')
    }) {
        return Err(path_error(
            "runtime_package_path_invalid_character",
            segment,
            "path segment contains a platform-invalid character",
        ));
    }
    let device_base = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    let reserved = matches!(device_base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || device_base.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || device_base.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        });
    if reserved {
        return Err(path_error(
            "runtime_package_path_windows_reserved_name",
            segment,
            "path segment uses a Windows reserved device name",
        ));
    }
    Ok(())
}

pub fn safe_join_runtime_package(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, RuntimePackagePathError> {
    let package_path = RuntimePackagePath::parse(relative_path.to_string())?;
    let canonical_root = fs::canonicalize(root).map_err(|error| RuntimePackagePathError {
        code: "runtime_package_root_unavailable",
        path: root.display().to_string(),
        message: error.to_string(),
    })?;
    let candidate = root.join(package_path.as_str());
    let mut existing = candidate.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| RuntimePackagePathError {
            code: "runtime_package_path_escape",
            path: relative_path.to_string(),
            message: "package path has no existing ancestor inside package root".to_string(),
        })?;
    }
    let canonical_existing =
        fs::canonicalize(existing).map_err(|error| RuntimePackagePathError {
            code: "runtime_package_path_canonicalize_failed",
            path: existing.display().to_string(),
            message: error.to_string(),
        })?;
    if !canonical_existing.starts_with(&canonical_root) {
        return Err(RuntimePackagePathError {
            code: "runtime_package_path_symlink_escape",
            path: relative_path.to_string(),
            message: format!(
                "existing path ancestor {} resolves outside package root {}",
                canonical_existing.display(),
                canonical_root.display()
            ),
        });
    }
    Ok(candidate)
}

fn validate_relative_package_path(value: &str) -> Result<(), RuntimePackagePathError> {
    if value.is_empty() {
        return Err(path_error(
            "runtime_package_path_empty",
            value,
            "package path cannot be empty",
        ));
    }
    if value.contains('\\') {
        return Err(path_error(
            "runtime_package_path_backslash",
            value,
            "package paths use forward slash separators only",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::RootDir
                    | Component::Prefix(_)
                    | Component::ParentDir
                    | Component::CurDir
            )
        })
    {
        return Err(path_error(
            "runtime_package_path_not_relative",
            value,
            "package path must be a normalized relative path",
        ));
    }
    for segment in value.split('/') {
        validate_package_path_segment(segment)?;
    }
    Ok(())
}

fn path_error(code: &'static str, path: &str, message: &str) -> RuntimePackagePathError {
    RuntimePackagePathError {
        code,
        path: path.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_package_path_rejects_absolute_traversal_and_windows_unsafe_paths() {
        for path in [
            "",
            "/absolute.json",
            "C:/absolute.json",
            "../escape.json",
            "a/./b.json",
            "a//b.json",
            "a\\b.json",
            "assets/CON.json",
            "assets/name. ",
            "assets/name?.json",
        ] {
            assert!(RuntimePackagePath::parse(path).is_err(), "path={path}");
        }
        assert!(RuntimePackagePath::parse("assets/ui/hud.json").is_ok());
    }

    #[test]
    fn runtime_package_path_rejects_windows_case_collisions() {
        let mut claims = RuntimePackagePathClaims::default();
        claims
            .claim(&RuntimePackagePath::parse("Textures/Ship.rgba8").unwrap())
            .unwrap();
        let error = claims
            .claim(&RuntimePackagePath::parse("textures/ship.rgba8").unwrap())
            .unwrap_err();
        assert_eq!(error.code, "runtime_package_path_collision");
    }

    #[test]
    fn runtime_package_path_safe_join_stays_inside_root() {
        let root =
            std::env::temp_dir().join(format!("aife-runtime-package-path-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let joined = safe_join_runtime_package(&root, "scenes/main.json").unwrap();
        assert!(joined.starts_with(&root));
        assert!(safe_join_runtime_package(&root, "../escape.json").is_err());
    }

    #[test]
    fn runtime_package_path_rejects_existing_symlink_escape_when_supported() {
        let base = std::env::temp_dir().join(format!(
            "aife-runtime-package-symlink-{}",
            std::process::id()
        ));
        let root = base.join("package");
        let outside = base.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = root.join("escape");
        if create_directory_symlink(&outside, &link).is_err() {
            return;
        }
        let error = safe_join_runtime_package(&root, "escape/payload.bin").unwrap_err();
        assert_eq!(error.code, "runtime_package_path_symlink_escape");
    }

    #[cfg(windows)]
    fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(unix)]
    fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }
}
