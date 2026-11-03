//! Launcher-owned setup, performed once before project simulation callbacks.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePlayerProjectStorage {
    pub base: PathBuf,
    pub profile: String,
}

fn default_binding() -> Result<NativePlayerProjectStorage, String> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or("CapabilityGap: no absolute per-user local data root")?;
        Ok(NativePlayerProjectStorage {
            base: base.join("OriginEngine/ProjectData"),
            profile: "default".into(),
        })
    }
    #[cfg(not(windows))]
    Err("CapabilityGap: project storage is not qualified on this platform".into())
}

pub(super) fn bind(
    request: &NativePlayerWindowRunRequest,
    package: &RuntimePackage,
    modules: &LinkedProjectRuntimeSet,
    hydrator: &mut RuntimeSceneHydrator,
    report: &mut NativeWindowHostReport,
) -> bool {
    let result = (|| {
        let required = modules
            .requires_project_storage(&package.manifest.project.runtime_module)
            .map_err(|error| error.message)?;
        if !required && request.project_storage.is_none() {
            return Ok(());
        }
        let binding = request
            .project_storage
            .clone()
            .map(Ok)
            .unwrap_or_else(default_binding)?;
        if !binding.base.is_absolute() {
            return Err("CapabilityGap: storage base must be absolute".into());
        }
        let normalize = |path: &std::path::Path| -> Result<String, String> {
            let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
            Ok(absolute
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .trim_end_matches('/')
                .to_lowercase()
                + "/")
        };
        let base = normalize(&binding.base)?;
        let package_root = normalize(&package.package_dir)?;
        if base.starts_with(&package_root) || package_root.starts_with(&base) {
            return Err(
                "CapabilityGap: storage base must be separate from the runtime package".into(),
            );
        }
        hydrator
            .instance_loader_mut()
            .bind_project_storage(
                &binding.base,
                &package.manifest.project.project_id,
                &binding.profile,
            )
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(()) => true,
        Err(error) => {
            report.present_status = NativeWindowPresentStatus::PackageFailed;
            report.diagnostics.push(NativeWindowHostDiagnostic::error(
                "project_storage.binding_failed",
                "storage",
                error,
            ));
            false
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn project_storage_376_player_binding_is_host_owned_version_stable_and_test_isolated() {
        let root = super::super::tests::temp_root("376-storage-binding");
        let package_dir = super::super::tests::write_minimal_runtime_package(&root, "storage");
        let mut package = load_runtime_package(&package_dir).value.unwrap();
        let modules = LinkedProjectRuntimeSet::explicit_empty();
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(&package_dir);
        let base = root.join("isolated-data");
        request.project_storage = Some(NativePlayerProjectStorage {
            base: base.clone(),
            profile: "test".into(),
        });
        let mut report = NativeWindowHostReport::base(&request);
        let mut first = RuntimeSceneHydrator::from_package(&package);
        assert!(bind(&request, &package, &modules, &mut first, &mut report));
        let mut competing = RuntimeSceneHydrator::from_package(&package);
        assert!(!bind(
            &request,
            &package,
            &modules,
            &mut competing,
            &mut report
        ));
        first.instance_loader_mut().close_project_storage();
        package.manifest.project.version = "updated-package".into();
        package.manifest.content_hash = Some("changed-hash".into());
        assert!(bind(
            &request,
            &package,
            &modules,
            &mut competing,
            &mut report
        ));
        competing.instance_loader_mut().close_project_storage();
        let names: Vec<_> = fs::read_dir(&base).unwrap().collect();
        assert_eq!(
            names.len(),
            1,
            "package version/hash must not select a second namespace"
        );
        request.project_storage.as_mut().unwrap().profile = "another-profile".into();
        assert!(bind(
            &request,
            &package,
            &modules,
            &mut competing,
            &mut report
        ));
        competing.instance_loader_mut().close_project_storage();
        assert_eq!(fs::read_dir(&base).unwrap().count(), 2);
        request.project_storage.as_mut().unwrap().base = package_dir.clone();
        assert!(!bind(
            &request,
            &package,
            &modules,
            &mut competing,
            &mut report
        ));
        assert!(!package_dir.join(".aife-lease").exists());
        package.package_dir = package_dir.canonicalize().unwrap();
        request.project_storage.as_mut().unwrap().base = package_dir.clone();
        assert!(
            !bind(&request, &package, &modules, &mut competing, &mut report),
            "ordinary base and verbatim package paths must compare as the same directory"
        );
    }
}
