use engine_runtime::runtime_package::{RuntimePackageManifest, RUNTIME_PACKAGE_SCHEMA_VERSION};
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIBE_FLAG: &str = "--describe-project-runtime-module";
const MODULE_PATH_FLAG: &str = "--project-runtime-dll";

/// Project-independent entrypoint. Execution stays on the existing packaged CLI path.
pub fn run_fixed_player_from_env() -> i32 {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some(DESCRIBE_FLAG) {
        return crate::run_from_env();
    }
    let result = std::env::current_exe()
        .map_err(|error| format!("current executable unavailable: {error}"))
        .and_then(|executable| describe_module(&args, &executable));
    match result {
        Ok(descriptor) => {
            println!("{descriptor}");
            0
        }
        Err(error) => {
            eprintln!("project runtime descriptor query failed: {error}");
            1
        }
    }
}

fn descriptor_path_argument(args: &[String]) -> Result<Option<PathBuf>, String> {
    match args {
        [flag] if flag == DESCRIBE_FLAG => Ok(None),
        [flag, path_flag, path]
            if flag == DESCRIBE_FLAG && path_flag == MODULE_PATH_FLAG && !path.is_empty() =>
        {
            Ok(Some(PathBuf::from(path)))
        }
        _ => Err(format!(
            "Usage: {DESCRIBE_FLAG} [{MODULE_PATH_FLAG} <path>]"
        )),
    }
}

fn resolve_module_path(executable: &Path, explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = if let Some(path) = explicit {
        // An explicit missing or invalid module must never select another project.
        path
    } else {
        let directory = executable.parent().ok_or("executable parent is missing")?;
        let package = directory.join("data/runtime_package");
        if package.exists() {
            let manifest_path = package.join("manifest.json");
            let bytes = fs::read(&manifest_path)
                .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
            let manifest: RuntimePackageManifest = serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
            let module_id = &manifest.project.runtime_module.module_id;
            if manifest.schema_version != RUNTIME_PACKAGE_SCHEMA_VERSION
                || module_id.trim().is_empty()
            {
                return Err(format!(
                    "{} has no supported project module identity",
                    manifest_path.display()
                ));
            }
            // The package only selects the DLL. Its descriptor is read from the DLL below.
            directory.join(crate::project_runtime_module_relative_path(module_id))
        } else {
            directory.join("project_runtime_module.dll")
        }
    };
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !resolved.is_file() {
        return Err(format!("{} is not a module file", path.display()));
    }
    Ok(resolved)
}

fn describe_module(args: &[String], executable: &Path) -> Result<String, String> {
    let path = resolve_module_path(executable, descriptor_path_argument(args)?)?;
    #[cfg(windows)]
    {
        let modules =
            engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll(
                &path,
            )
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        let descriptor = modules
            .only_descriptor()
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        serde_json::to_string(descriptor).map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("project runtime DLL loading is only supported on Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aife-fixed-player-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn executable(&self) -> PathBuf {
            self.0.join("Game.exe")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn fixed_player_descriptor_arguments_reject_unknown_missing_and_extra_values() {
        let parse = |args: &[&str]| {
            descriptor_path_argument(&args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>())
        };
        assert_eq!(parse(&[DESCRIBE_FLAG]).unwrap(), None);
        assert_eq!(
            parse(&[DESCRIBE_FLAG, MODULE_PATH_FLAG, "project module.dll"]).unwrap(),
            Some(PathBuf::from("project module.dll"))
        );
        for args in [
            vec![DESCRIBE_FLAG, MODULE_PATH_FLAG],
            vec![DESCRIBE_FLAG, "--unknown", "module.dll"],
            vec![DESCRIBE_FLAG, MODULE_PATH_FLAG, ""],
            vec![DESCRIBE_FLAG, MODULE_PATH_FLAG, "module.dll", "extra"],
        ] {
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn fixed_player_explicit_missing_module_never_falls_back_to_sibling() {
        let fixture = Fixture::new();
        let sibling = fixture.0.join("project_runtime_module.dll");
        fs::write(&sibling, b"sibling module").unwrap();
        assert_eq!(
            resolve_module_path(&fixture.executable(), None).unwrap(),
            sibling.canonicalize().unwrap()
        );
        let missing = fixture.0.join("missing-project.dll");
        assert!(resolve_module_path(&fixture.executable(), Some(missing)).is_err());
        fs::remove_file(sibling).unwrap();
        assert!(resolve_module_path(&fixture.executable(), None).is_err());
    }

    #[test]
    fn fixed_player_packaged_identity_selects_only_its_actual_dll_path() {
        let fixture = Fixture::new();
        let package = fixture.0.join("data/runtime_package");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir(fixture.0.join("data/bin")).unwrap();
        fs::write(
            package.join("manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": "runtime-package.v2", "packageMode": "debug-readable",
                "project": { "projectId": "fixture", "name": "Fixture", "version": "1",
                    "runtimeModule": { "moduleId": "fixture.project-runtime", "interfaceVersion": "v1", "aotContentDigest": "fixture" } },
                "activeSceneId": "main", "scenes": [],
                "assets": {"path": "assets.json", "assetCount": 0},
                "rules": {"path": "rules.json", "mode": "none"},
                "input": {"path": "input.json", "defaultMappingId": "none", "mappingCount": 0}
            })).unwrap(),
        ).unwrap();
        let module = fixture.0.join("data/bin/fixture_project_runtime.dll");
        fs::write(&module, b"not a DLL").unwrap();
        fs::write(
            fixture.0.join("project_runtime_module.dll"),
            b"wrong project",
        )
        .unwrap();
        assert_eq!(
            resolve_module_path(&fixture.executable(), None).unwrap(),
            module.canonicalize().unwrap()
        );
        // A manifest never substitutes for a descriptor from a valid native module.
        assert!(describe_module(&[DESCRIBE_FLAG.into()], &fixture.executable()).is_err());
        fs::remove_file(module).unwrap();
        assert!(resolve_module_path(&fixture.executable(), None).is_err());
        fs::write(package.join("manifest.json"), b"invalid manifest").unwrap();
        assert!(resolve_module_path(&fixture.executable(), None).is_err());
    }
}
