use runtime_cli::{
    project_runtime_module_relative_path, runtime_package_digest, semantic_file_digest,
    validate_desktop_dev_manifest, validate_desktop_dev_package, verify_exported_player_process,
    DesktopPackageManifest, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MODULE_ID: &str = "fixture.desktop.runtime";
const MODULE_PATH: &str = "data/bin/fixture_desktop_runtime.dll";
const PAYLOAD_PATH: &str = "data/runtime_package/scenes/main.json";

#[test]
fn desktop_dev_missing_identity_is_rejected_before_launch() {
    let fixture = Fixture::new("old-dev");
    fixture.write_manifest_value(&json!({
        "schemaVersion": "desktop-package-manifest.v1",
        "target": "windows",
        "profile": "dev",
        "packageDir": fixture.package,
        "runtimePackageDir": fixture.package.join("data/runtime_package"),
        "reportsDir": fixture.package.join("reports"),
        "playerExecutableStatus": "copied"
    }));

    assert_rejected_before_spawn(&fixture.package, "desktop_dev_identity_missing");
}

#[test]
fn desktop_dev_same_source_files_pass_static_validation() {
    let fixture = Fixture::new("valid");
    let manifest = validate_desktop_dev_package(&fixture.package).unwrap();

    assert_eq!(manifest, fixture.manifest());
    assert_eq!(project_runtime_module_relative_path(MODULE_ID), MODULE_PATH);
    validate_desktop_dev_manifest(&fixture.package, &manifest).unwrap();
}

#[test]
fn desktop_dev_moved_package_uses_current_root_without_rewriting_provenance() {
    let mut fixture = Fixture::new("moved");
    let original_package = fixture.package.clone();
    let original_manifest = fs::read(fixture.package.join("package-manifest.json")).unwrap();
    let original_report = b"original export report with unchanged provenance";
    fs::write(
        fixture.package.join("reports/desktop-export-report.json"),
        original_report,
    )
    .unwrap();
    let moved = fixture.root.join("moved-package");
    fs::rename(&fixture.package, &moved).unwrap();
    fixture.package = moved;

    assert!(!original_package.exists());
    let manifest = validate_desktop_dev_package(&fixture.package).unwrap();
    assert_eq!(manifest.package_dir, original_package.display().to_string());
    assert_eq!(
        fs::read(fixture.package.join("package-manifest.json")).unwrap(),
        original_manifest
    );
    assert_eq!(
        fs::read(fixture.package.join("reports/desktop-export-report.json")).unwrap(),
        original_report
    );
}

#[test]
fn desktop_dev_replaced_executable_or_dll_is_rejected_before_spawn() {
    for path in ["Game.exe", "engine_runtime.dll", MODULE_PATH] {
        let fixture = Fixture::new("replaced-runtime-file");
        fs::write(fixture.package.join(path), b"different build bytes").unwrap();

        assert_rejected_before_spawn(&fixture.package, "desktop_dev_file_hash_mismatch");
    }
}

#[test]
fn desktop_dev_missing_executable_or_dll_is_rejected_before_spawn() {
    for path in ["Game.exe", "engine_runtime.dll", MODULE_PATH] {
        let fixture = Fixture::new("missing-runtime-file");
        fs::remove_file(fixture.package.join(path)).unwrap();

        assert_rejected_before_spawn(&fixture.package, "desktop_dev_file_missing");
    }
}

#[test]
fn desktop_dev_changed_or_removed_runtime_payload_is_rejected_before_spawn() {
    for remove_payload in [false, true] {
        let fixture = Fixture::new("changed-payload");
        if remove_payload {
            fs::remove_file(fixture.package.join(PAYLOAD_PATH)).unwrap();
        } else {
            fs::write(fixture.package.join(PAYLOAD_PATH), b"different scene").unwrap();
        }

        assert_rejected_before_spawn(
            &fixture.package,
            "desktop_dev_runtime_package_digest_mismatch",
        );
    }
}

#[test]
fn desktop_dev_stale_module_descriptor_is_rejected_before_spawn() {
    for field in ["interfaceVersion", "aotContentDigest"] {
        let fixture = Fixture::new("stale-module");
        let mut manifest = fixture.manifest_value();
        manifest["playerModuleDescriptor"][field] = json!("stale-value");
        fixture.write_manifest_value(&manifest);

        assert_rejected_before_spawn(&fixture.package, "desktop_dev_module_mismatch");
    }
}

#[test]
fn desktop_dev_runtime_package_module_id_mismatch_is_rejected_before_spawn() {
    let fixture = Fixture::new("different-runtime-module");
    let path = fixture.package.join("data/runtime_package/manifest.json");
    let mut runtime: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    runtime["project"]["runtimeModule"]["moduleId"] = json!("another.project.runtime");
    fs::write(&path, serde_json::to_vec_pretty(&runtime).unwrap()).unwrap();
    let mut manifest = fixture.manifest_value();
    manifest["runtimePackageDigest"] =
        json!(runtime_package_digest(&fixture.package.join("data/runtime_package")).unwrap());
    fixture.write_manifest_value(&manifest);

    assert_rejected_before_spawn(&fixture.package, "desktop_dev_module_mismatch");
}

#[test]
fn desktop_dev_each_missing_identity_field_is_rejected_before_spawn() {
    for field in [
        "playerArtifactHash",
        "engineRuntimeHash",
        "projectRuntimeModuleHash",
        "runtimePackageDigest",
        "playerModuleDescriptor",
    ] {
        let fixture = Fixture::new("missing-identity-field");
        let mut manifest = fixture.manifest_value();
        manifest.as_object_mut().unwrap().remove(field);
        fixture.write_manifest_value(&manifest);

        assert_rejected_before_spawn(&fixture.package, "desktop_dev_identity_missing");
    }
}

#[test]
fn desktop_dev_malformed_manifest_is_not_accepted_as_legacy_dev() {
    let fixture = Fixture::new("malformed-manifest");
    fs::write(fixture.package.join("package-manifest.json"), b"{invalid").unwrap();

    assert_eq!(
        validate_desktop_dev_package(&fixture.package)
            .unwrap_err()
            .code,
        "desktop_dev_manifest_invalid"
    );
    // Invalid JSON cannot identify its package kind; the outer resolver must
    // reject it before selecting either the dev or release contract.
    assert_verifier_rejected_before_spawn(&fixture.package, "package_manifest_invalid");
}

#[test]
fn desktop_dev_duplicate_managed_directories_are_rejected_before_spawn() {
    for path in ["data/data/runtime_package", "reports/reports"] {
        let fixture = Fixture::new("duplicate-directory");
        fs::create_dir_all(fixture.package.join(path)).unwrap();
        fs::write(fixture.package.join(path).join("old-output.json"), b"{}").unwrap();

        assert_rejected_before_spawn(&fixture.package, "desktop_dev_layout_invalid");
    }
}

#[test]
fn desktop_dev_extra_project_dll_is_rejected_before_spawn() {
    let fixture = Fixture::new("extra-project-dll");
    fs::write(
        fixture.package.join("data/bin/old_project_runtime.dll"),
        b"old DLL",
    )
    .unwrap();

    assert_rejected_before_spawn(&fixture.package, "desktop_dev_layout_invalid");
}

#[test]
fn desktop_dev_legacy_loader_candidates_are_rejected_before_spawn() {
    for relative in [
        "data/project_runtime.dll",
        "data/runtime_package/project_runtime.dll",
    ] {
        let fixture = Fixture::new("legacy-loader-candidate");
        fs::write(fixture.package.join(relative), b"unbound legacy DLL").unwrap();
        assert_rejected_before_spawn(&fixture.package, "desktop_dev_layout_invalid");
    }
}

#[test]
fn desktop_dev_readme_license_and_reports_are_allowed() {
    let fixture = Fixture::new("permitted-metadata");
    for path in [
        "README.md",
        "LICENSE.txt",
        "reports/desktop-export-report.json",
        "reports/verification-result.json",
    ] {
        fs::write(fixture.package.join(path), b"retained metadata").unwrap();
    }

    validate_desktop_dev_package(&fixture.package).unwrap();
}

fn assert_rejected_before_spawn(package: &Path, code: &str) {
    let diagnostic = validate_desktop_dev_package(package).unwrap_err();
    assert_eq!(diagnostic.code, code, "{diagnostic:#?}");
    assert_eq!(diagnostic.severity, "error");
    assert!(!diagnostic.message.is_empty());
    assert!(diagnostic.path.is_some(), "{diagnostic:#?}");

    assert_verifier_rejected_before_spawn(package, code);
}

fn assert_verifier_rejected_before_spawn(package: &Path, code: &str) {
    let report = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
        exported_package_dir: package.to_path_buf(),
        mode: "headless-gate".into(),
        frame_limit: 1,
        report_path: None,
        timeout_ms: 100,
        screenshot: false,
        screenshot_path: None,
    });
    assert_eq!(
        report.status,
        ExportedPlayerProcessVerificationStatus::Failed,
        "{report:#?}"
    );
    assert_eq!(report.exit_code, Some(1), "{report:#?}");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "{report:#?}"
    );
    assert_eq!(report.process_id, None, "{report:#?}");
    assert_eq!(report.process_spawn_error, None, "{report:#?}");
    assert_eq!(report.process_elapsed_ms, 0, "{report:#?}");
    assert_eq!(report.process_exit_reason, "not_started", "{report:#?}");
}

struct Fixture {
    root: PathBuf,
    package: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aife-335-dev-{label}-{}-{nonce}",
            std::process::id()
        ));
        let package = root.join("package");
        for directory in ["data/bin", "data/runtime_package/scenes", "reports"] {
            fs::create_dir_all(package.join(directory)).unwrap();
        }
        fs::write(
            package.join("Game.exe"),
            b"not an executable; do not launch",
        )
        .unwrap();
        fs::write(package.join("engine_runtime.dll"), b"engine DLL bytes").unwrap();
        fs::write(package.join(MODULE_PATH), b"project DLL bytes").unwrap();
        fs::write(package.join(PAYLOAD_PATH), br#"{"scene":"fixture"}"#).unwrap();
        let descriptor = json!({
            "moduleId": MODULE_ID,
            "interfaceVersion": "project-runtime-module.v2",
            "aotContentDigest": "fixture-aot-content"
        });
        fs::write(
            package.join("data/runtime_package/manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": "runtime-package.v2",
                "project": { "runtimeModule": descriptor }
            }))
            .unwrap(),
        )
        .unwrap();
        let fixture = Self { root, package };
        fixture.write_manifest_value(&json!({
            "schemaVersion": "desktop-package-manifest.v1",
            "target": "windows",
            "profile": "dev",
            "packageDir": fixture.package,
            "runtimePackageDir": fixture.package.join("data/runtime_package"),
            "reportsDir": fixture.package.join("reports"),
            "playerExecutable": fixture.package.join("Game.exe"),
            "playerExecutableStatus": "copied",
            "playerArtifactBuildReportPath": fixture.package.join("reports/player-build.json"),
            "playerArtifactHash": semantic_file_digest(&fixture.package.join("Game.exe")).unwrap(),
            "playerModuleDescriptor": descriptor,
            "engineRuntimeHash": semantic_file_digest(&fixture.package.join("engine_runtime.dll")).unwrap(),
            "projectRuntimeModuleHash": semantic_file_digest(&fixture.package.join(MODULE_PATH)).unwrap(),
            "runtimePackageDigest": runtime_package_digest(&fixture.package.join("data/runtime_package")).unwrap()
        }));
        fixture
    }

    fn manifest(&self) -> DesktopPackageManifest {
        serde_json::from_value(self.manifest_value()).unwrap()
    }

    fn manifest_value(&self) -> Value {
        serde_json::from_slice(&fs::read(self.package.join("package-manifest.json")).unwrap())
            .unwrap()
    }

    fn write_manifest_value(&self, value: &Value) {
        fs::write(
            self.package.join("package-manifest.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
